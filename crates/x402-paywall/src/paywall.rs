use std::fmt::Display;

use bon::Builder;
use http::{Request, Response, StatusCode};
use x402_kit::{
    core::Resource,
    facilitator::{Facilitator, SupportedResponse},
    transport::{Accepts, PaymentPayload, PaymentRequired},
    types::{Base64EncodedHeader, Extension, Record, X402V2},
};

use crate::{
    errors::{ErrorResponse, ErrorResponseHeader},
    processor::{PaymentState, RequestProcessor},
};

/// A http paywall that uses a facilitator to verify and settle payments.
#[derive(Builder, Debug, Clone)]
pub struct PayWall<F: Facilitator> {
    /// The facilitator to use for payment verification and settlement.
    pub facilitator: F,
    /// The resource this paywall serves.
    pub resource: Resource,
    /// The accepted payment requirements.
    #[builder(into)]
    pub accepts: Accepts,
    /// Additional extensions to use.
    #[builder(default)]
    pub extensions: Record<Extension>,
}

impl<F: Facilitator> PayWall<F> {
    /// Entrypoint of an X402 payment flow.
    ///
    /// Process an incoming request and extract payment information.
    ///
    /// Returns a [`RequestProcessor`] on success for further processing.
    pub fn process_request<'pw, Req>(
        &'pw self,
        request: Request<Req>,
    ) -> Result<RequestProcessor<'pw, F, Req>, ErrorResponse> {
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

        let payload = PaymentPayload::try_from(payment_signature.clone()).map_err(|err| {
            self.invalid_payment(&format!("Failed to parse PAYMENT-SIGNATURE header: {err}"))
        })?;

        let initial_state = PaymentState {
            verified: None,
            settled: None,
            required_extensions: self.extensions.to_owned(),
            payload_extensions: payload.extensions.clone(),
        };

        let selected = self
            .accepts
            .clone()
            .into_iter()
            // Match a PaymentRequirements with PartialEq
            .find(|a| a == &payload.accepted)
            .ok_or_else(|| self.invalid_payment("PaymentRequirements in payload not accepted"))?;

        Ok(RequestProcessor {
            paywall: self,
            selected,
            request,
            payload,
            payment_state: initial_state,
        })
    }

    /// Standard payment handling flow.
    ///
    /// This handler will **update** the accepted payment requirements from the facilitator,
    /// **verify** the payment, **run** the provided resource handler, and **settle** the payment on success.
    pub async fn handle_payment<Fun, Fut, Req, Res>(
        self,
        request: Request<Req>,
        handler: Fun,
    ) -> Result<Response<Res>, ErrorResponse>
    where
        Fun: FnOnce(Request<Req>) -> Fut,
        Fut: Future<Output = Response<Res>>,
    {
        let response = self
            .update_accepts()
            .await?
            .process_request(request)?
            .verify()
            .await?
            .run_handler(handler)
            .await?
            .settle_on_success()
            .await?
            .response();

        Ok(response)
    }

    /// Update the accepted payment requirements based on the facilitator's supported kinds.
    pub async fn update_accepts(mut self) -> Result<Self, ErrorResponse> {
        let supported = self
            .facilitator
            .supported()
            .await
            .map_err(|err| self.server_error(&err))?;
        self.accepts = filter_supported_accepts(&supported, self.accepts.to_owned());

        Ok(self)
    }

    /// Payment needed to access resource
    pub fn payment_required(&self) -> ErrorResponse {
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

        ErrorResponse {
            status: StatusCode::PAYMENT_REQUIRED,
            header: ErrorResponseHeader::PaymentRequired(header),
            body: payment_required,
        }
    }

    /// Malformed payment payload or requirements
    pub fn invalid_payment(&self, reason: impl Display) -> ErrorResponse {
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

        ErrorResponse {
            status: StatusCode::BAD_REQUEST,
            header: ErrorResponseHeader::PaymentResponse(header),
            body: payment_required,
        }
    }

    /// Payment verification or settlement failed
    pub fn payment_failed(&self, reason: impl Display) -> ErrorResponse {
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

        ErrorResponse {
            status: StatusCode::PAYMENT_REQUIRED,
            header: ErrorResponseHeader::PaymentResponse(header),
            body: payment_required,
        }
    }

    /// Internal server error during payment processing
    pub fn server_error(&self, reason: impl Display) -> ErrorResponse {
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

        ErrorResponse {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            header: ErrorResponseHeader::PaymentResponse(header),
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
