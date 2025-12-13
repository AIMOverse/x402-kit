use std::fmt::Display;

use bon::Builder;
use http::{HeaderValue, Request, Response, StatusCode};
use x402_kit::{
    core::Resource,
    facilitator::{
        Facilitator, PaymentRequest, SettleResult, SettleSuccess, SupportedResponse, VerifyResult,
        VerifyValid,
    },
    transport::{Accepts, PaymentPayload, PaymentRequired, SettlementResponse},
    types::{Base64EncodedHeader, Extension, Record, X402V2},
};

#[derive(Builder, Debug, Clone)]
pub struct PayWall<F: Facilitator> {
    pub facilitator: F,
    #[builder(default)]
    pub config: PayWallConfig,
    pub resource: Resource,
    pub accepts: Accepts,
    #[builder(default)]
    pub extensions: Record<Extension>,
}

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

#[derive(Debug, Clone)]
pub struct PaymentStatus {
    pub verified: Option<VerifyValid>,
    pub settled: Option<SettleSuccess>,
    pub extensions: Record<Extension>,
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

            let payment_status = PaymentStatus {
                verified: valid,
                settled: Some(settled.clone()),
                extensions: self.extensions.to_owned(),
            };

            request.extensions_mut().insert(payment_status.clone());

            let response = handler(request).await;
            (response, settled)
        } else {
            // Proceed first, then settle
            let payment_status = PaymentStatus {
                verified: valid,
                settled: None,
                extensions: self.extensions.to_owned(),
            };

            request.extensions_mut().insert(payment_status.clone());

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
