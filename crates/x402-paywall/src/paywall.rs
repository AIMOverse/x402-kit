use std::fmt::Display;

use bon::Builder;
use bytes::Bytes;
use http::{HeaderName, HeaderValue, Request, Response, StatusCode};
use http_body_util::Full;
use x402_kit::{
    core::Resource,
    facilitator::{
        Facilitator, PaymentRequest, SettleResult, SettleSuccess, SupportedResponse, VerifyResult,
        VerifyValid,
    },
    transport::{Accepts, PaymentPayload, PaymentRequired, SettlementResponse},
    types::{Base64EncodedHeader, Extension, Record, X402V2},
};

/// A http paywall that uses a facilitator to verify and settle payments.
#[derive(Builder, Debug, Clone)]
pub struct PayWall<F: Facilitator> {
    /// The facilitator to use for payment verification and settlement.
    pub facilitator: F,
    /// Paywall behavior configuration.
    #[builder(default)]
    pub config: PayWallConfig,
    /// The resource this paywall serves.
    pub resource: Resource,
    /// The accepted payment requirements.
    #[builder(into)]
    pub accepts: Accepts,
    /// Additional extensions to use.
    #[builder(default)]
    pub extensions: Record<Extension>,
}

/// Paywall configuration options.
///
/// The default behavior is to:
/// - Update accepted payment kinds from the facilitator
/// - Verify payments with the facilitator
/// - Settle payments before proceeding to the handler
/// - Not skip settlement on any response status codes
#[derive(Builder, Debug, Clone, Default)]
pub struct PayWallConfig {
    /// Skip updating supported payment kinds from the facilitator
    #[builder(default, with = || true)]
    pub skip_update_accepts: bool,

    /// Skip verifying the payment with the facilitator
    #[builder(default, with = || true)]
    pub skip_verify: bool,

    /// Skip settling the payment with the facilitator
    #[builder(default, with = || true)]
    pub settle_before_access: bool,

    /// HTTP status codes that will skip settlement when returned from the handler
    #[builder(default, with = |i: impl IntoIterator<Item = StatusCode>| i.into_iter().collect())]
    pub skip_settle_on_status: Vec<StatusCode>,
}

/// The state of a payment processed by the paywall when accessing the resource handler.
#[derive(Debug, Clone)]
pub struct PaymentState {
    /// Verification result, if verification was performed.
    pub verified: Option<VerifyValid>,
    /// Settlement result, if settlement was performed.
    pub settled: Option<SettleSuccess>,
    /// All extensions info provided by the paywall.
    pub extensions: Record<Extension>,
    /// All extensions info provided by the signer.
    pub signer_extensions: Record<Extension>,
}

#[derive(Debug, Clone)]
pub struct PayWallErrorResponse {
    pub status: StatusCode,
    pub header: PayWallErrorHeader,
    pub body: PaymentRequired,
}

#[derive(Debug, Clone)]
pub enum PayWallErrorHeader {
    PaymentRequired(Base64EncodedHeader),
    PaymentResponse(Base64EncodedHeader),
}

impl PayWallErrorHeader {
    pub fn header_name(&self) -> HeaderName {
        match self {
            PayWallErrorHeader::PaymentRequired(_) => HeaderName::from_static("PAYMENT-REQUIRED"),
            PayWallErrorHeader::PaymentResponse(_) => HeaderName::from_static("PAYMENT-RESPONSE"),
        }
    }

    pub fn header_value(self) -> Option<(HeaderName, HeaderValue)> {
        match self {
            PayWallErrorHeader::PaymentRequired(Base64EncodedHeader(s)) => {
                HeaderValue::from_str(&s)
                    .ok()
                    .map(|v| (HeaderName::from_static("PAYMENT-REQUIRED"), v))
            }
            PayWallErrorHeader::PaymentResponse(Base64EncodedHeader(s)) => {
                HeaderValue::from_str(&s)
                    .ok()
                    .map(|v| (HeaderName::from_static("PAYMENT-RESPONSE"), v))
            }
        }
    }
}
impl From<PayWallErrorResponse> for Response<Full<Bytes>> {
    fn from(value: PayWallErrorResponse) -> Self {
        let body = match serde_json::to_vec(&value.body) {
            Ok(b) => b,
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::error!(
                    "Failed to serialize PayWallErrorResponse body to JSON bytes: {err}"
                );

                let mut response = Response::new(Full::new(Bytes::from_static(
                    b"Failed to serialize PayWallErrorResponse body to JSON bytes",
                )));
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;

                return response;
            }
        };

        // Build the response
        let mut response = Response::new(Full::new(Bytes::from_iter(body)));
        *response.status_mut() = value.status;
        if let Some((name, val)) = value.header.header_value() {
            response.headers_mut().insert(name, val);
        }
        response
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for PayWallErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let mut response = (self.status, axum::extract::Json(self.body)).into_response();
        if let Some((name, val)) = self.header.header_value() {
            response.headers_mut().insert(name, val);
        }
        response
    }
}

impl<F: Facilitator> PayWall<F> {
    pub async fn handle_payment<Fun, Fut, Req, Res>(
        &self,
        mut request: Request<Req>,
        handler: Fun,
    ) -> Result<Response<Res>, PayWallErrorResponse>
    where
        Fun: FnOnce(Request<Req>) -> Fut,
        Fut: Future<Output = Response<Res>>,
    {
        let payment_signature = request
            .headers()
            .get("PAYMENT-SIGNATURE")
            .ok_or_else(|| self.payment_required())
            .and_then(|h| {
                h.to_str().map_err(|err| {
                    self.invalid_payment(&format!(
                        "Failed to decode PAYMENT-SIGNATURE header: {err}"
                    ))
                })
            })
            .map(|s| Base64EncodedHeader(s.to_string()))?;

        let payment_payload =
            PaymentPayload::try_from(payment_signature.clone()).map_err(|err| {
                self.invalid_payment(&format!("Failed to parse PAYMENT-SIGNATURE header: {err}"))
            })?;

        let accepts = if self.config.skip_update_accepts {
            self.accepts.to_owned()
        } else {
            self.updated_accepts().await?
        };

        let selected = accepts
            .into_iter()
            // Match a PaymentRequirements with PartialEq
            .find(|a| a == &payment_payload.accepted)
            .ok_or_else(|| self.invalid_payment("PaymentRequirements in payload not accepted"))?;

        let valid = if !self.config.skip_verify {
            // Verify payment with facilitator
            let response = self
                .facilitator
                .verify(PaymentRequest {
                    payment_signature: payment_signature.clone(),
                    payment_payload: payment_payload.clone(),
                    payment_requirements: selected.clone(),
                })
                .await
                .map_err(|err| self.server_error(&format!("Failed to verify payment: {err}")))?;

            let valid = match response {
                VerifyResult::Valid(v) => v,
                VerifyResult::Invalid(iv) => {
                    return Err(self.payment_failed(iv.invalid_reason));
                }
            };

            #[cfg(feature = "tracing")]
            tracing::debug!("Payment verified: payer='{}'", valid.payer);

            Some(valid)
        } else {
            None
        };

        // Take ownership of signer extensions from payload
        let signer_extensions = payment_payload.extensions.clone();

        // Handling different settlement strategies
        let (mut response, settled) = if self.config.settle_before_access {
            // Settle payment with facilitator
            let settlement = self
                .facilitator
                .settle(PaymentRequest {
                    payment_signature,
                    payment_payload,
                    payment_requirements: selected,
                })
                .await
                .map_err(|err| self.server_error(&format!("Failed to settle payment: {err}")))?;

            let settled = match settlement {
                SettleResult::Success(s) => s,
                SettleResult::Failed(f) => {
                    return Err(self.payment_failed(f.error_reason));
                }
            };

            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Payment settled: payer='{}', transaction='{}', network='{}'",
                settled.payer,
                settled.transaction,
                settled.network
            );

            let payment_status = PaymentState {
                verified: valid,
                settled: Some(settled.clone()),
                extensions: self.extensions.to_owned(),
                signer_extensions,
            };

            request.extensions_mut().insert(payment_status);

            let response = handler(request).await;
            (response, settled)
        } else {
            // Proceed first, then settle
            let payment_status = PaymentState {
                verified: valid,
                settled: None,
                extensions: self.extensions.to_owned(),
                signer_extensions,
            };

            request.extensions_mut().insert(payment_status);

            let response = handler(request).await;

            // Check if we should skip settling based on response status
            if self
                .config
                .skip_settle_on_status
                .contains(&response.status())
            {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Skipping settlement due to response status: {}",
                    response.status()
                );

                return Ok(response);
            }
            // Settle payment with facilitator
            let settlement = self
                .facilitator
                .settle(PaymentRequest {
                    payment_signature,
                    payment_payload,
                    payment_requirements: selected,
                })
                .await
                .map_err(|err| self.server_error(&format!("Failed to settle payment: {err}")))?;

            let settled = match settlement {
                SettleResult::Success(s) => s,
                SettleResult::Failed(f) => {
                    return Err(self.payment_failed(f.error_reason));
                }
            };

            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Payment settled: payer='{}', transaction='{}', network='{}'",
                settled.payer,
                settled.transaction,
                settled.network
            );

            (response, settled)
        };

        let settlement_response = SettlementResponse {
            success: true,
            payer: settled.payer,
            transaction: settled.transaction,
            network: settled.network,
        };

        let header = Base64EncodedHeader::try_from(settlement_response)
            .inspect_err(|err| {
                tracing::warn!("Failed to encode PAYMENT-RESPONSE header: {err}; skipping")
            })
            .ok()
            .and_then(|h| {
                HeaderValue::from_str(&h.0)
                    .inspect_err(|err| {
                        tracing::warn!("Failed to encode PAYMENT-RESPONSE header: {err}; skipping")
                    })
                    .ok()
            });

        if let Some(header) = header {
            response.headers_mut().insert("PAYMENT-RESPONSE", header);
        }

        Ok(response)
    }

    /// Get updated accepts from facilitator
    pub async fn updated_accepts(&self) -> Result<Accepts, PayWallErrorResponse> {
        let supported = self
            .facilitator
            .supported()
            .await
            .map_err(|err| self.server_error(&err))?;
        Ok(filter_supported_accepts(
            &supported,
            self.accepts.to_owned(),
        ))
    }

    /// Payment needed to access resource
    pub fn payment_required(&self) -> PayWallErrorResponse {
        let payment_required = PaymentRequired {
            x402_version: X402V2,
            error: "PAYMENT-SIGNATURE header is required".to_string(),
            resource: self.resource.to_owned().into(),
            accepts: self.accepts.to_owned(),
            extensions: self.extensions.to_owned(),
        };

        let header = Base64EncodedHeader::try_from(payment_required.clone()).unwrap_or(
            Base64EncodedHeader("Failed to encode base64 PaymentRequired payload".to_string()),
        );

        PayWallErrorResponse {
            status: StatusCode::PAYMENT_REQUIRED,
            header: PayWallErrorHeader::PaymentRequired(header),
            body: payment_required,
        }
    }

    /// Malformed payment payload or requirements
    pub fn invalid_payment(&self, reason: impl Display) -> PayWallErrorResponse {
        let payment_required = PaymentRequired {
            x402_version: X402V2,
            error: reason.to_string(),
            resource: self.resource.to_owned().into(),
            accepts: self.accepts.to_owned(),
            extensions: self.extensions.to_owned(),
        };

        let header = Base64EncodedHeader::try_from(payment_required.clone()).unwrap_or(
            Base64EncodedHeader("Failed to encode base64 PaymentRequired payload".to_string()),
        );

        PayWallErrorResponse {
            status: StatusCode::BAD_REQUEST,
            header: PayWallErrorHeader::PaymentResponse(header),
            body: payment_required,
        }
    }

    /// Payment verification or settlement failed
    pub fn payment_failed(&self, reason: impl Display) -> PayWallErrorResponse {
        let payment_required = PaymentRequired {
            x402_version: X402V2,
            error: reason.to_string(),
            resource: self.resource.to_owned().into(),
            accepts: self.accepts.to_owned(),
            extensions: self.extensions.to_owned(),
        };

        let header = Base64EncodedHeader::try_from(payment_required.clone()).unwrap_or(
            Base64EncodedHeader("Failed to encode base64 PaymentRequired payload".to_string()),
        );

        PayWallErrorResponse {
            status: StatusCode::PAYMENT_REQUIRED,
            header: PayWallErrorHeader::PaymentResponse(header),
            body: payment_required,
        }
    }

    /// Internal server error during payment processing
    pub fn server_error(&self, reason: impl Display) -> PayWallErrorResponse {
        let payment_required = PaymentRequired {
            x402_version: X402V2,
            error: reason.to_string(),
            resource: self.resource.to_owned().into(),
            accepts: self.accepts.to_owned(),
            extensions: self.extensions.to_owned(),
        };

        let header = Base64EncodedHeader::try_from(payment_required.clone()).unwrap_or(
            Base64EncodedHeader("Failed to encode base64 PaymentRequired payload".to_string()),
        );

        PayWallErrorResponse {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            header: PayWallErrorHeader::PaymentResponse(header),
            body: payment_required,
        }
    }
}

/// Filters the payment requirements based on the supported kinds from the facilitator.
///
/// Returns only the payment requirements that are supported by the facilitator with updated extra fields.
pub fn filter_supported_accepts(supported: &SupportedResponse, accepts: Accepts) -> Accepts {
    accepts
        .into_iter()
        .filter_map(|mut pr| {
            supported
                .kinds
                .iter()
                .find(|kind| kind.scheme == pr.scheme && kind.network == pr.network)
                .map(|s| {
                    // Update extra field if present
                    if s.extra.is_some() {
                        pr.extra = s.extra.clone();
                    }
                    pr
                })
        })
        .collect()
}
