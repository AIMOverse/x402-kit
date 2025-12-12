use bon::Builder;
use http::StatusCode;

use crate::{
    core::Resource,
    facilitator::Facilitator,
    transport::{Accepts, PaymentRequired},
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
    pub skip_supported: bool,

    /// Skip verifying the payment with the facilitator
    #[builder(default, with = || true)]
    pub skip_verify: bool,

    /// Skip settling the payment with the facilitator
    #[builder(default, with = || true)]
    pub settle_before_access: bool,
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
    /// Payment needed to access resource
    pub fn payment_required_response(&self) -> PayWallErrorResponse {
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
    pub fn invalid_payment_response(&self, reason: &str) -> PayWallErrorResponse {
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
    pub fn payment_failed_response(&self, reason: &str) -> PayWallErrorResponse {
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
    pub fn server_error_response(&self, reason: &str) -> PayWallErrorResponse {
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
