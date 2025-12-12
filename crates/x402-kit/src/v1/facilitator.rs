use serde::{Deserialize, Serialize};

use crate::{
    types::{AnyJson, Base64EncodedHeader, X402Version},
    v1::transport::{PaymentPayload, PaymentRequirements, PaymentResponse},
};

#[derive(Debug, Clone)]
pub struct FacilitatorPaymentRequest {
    pub payload: FacilitatorPaymentRequestPayload,
    pub x_payment_header: Base64EncodedHeader,
}

#[derive(Debug, Clone)]
pub struct FacilitatorPaymentRequestPayload {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

#[derive(Debug, Clone)]
pub enum FacilitatorVerifyResponse {
    Valid(FacilitatorVerifyValid),
    Invalid(FacilitatorVerifyInvalid),
}

impl FacilitatorVerifyResponse {
    pub fn is_valid(&self) -> bool {
        matches!(self, FacilitatorVerifyResponse::Valid(_))
    }

    pub fn valid(valid: FacilitatorVerifyValid) -> Self {
        FacilitatorVerifyResponse::Valid(valid)
    }

    pub fn invalid(invalid: FacilitatorVerifyInvalid) -> Self {
        FacilitatorVerifyResponse::Invalid(invalid)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacilitatorVerifyValid {
    pub payer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacilitatorVerifyInvalid {
    pub invalid_reason: String,
    pub payer: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FacilitatorSettleResponse {
    Success(FacilitatorSettleSuccess),
    Failed(FacilitatorSettleFailed),
}

impl FacilitatorSettleResponse {
    pub fn is_success(&self) -> bool {
        matches!(self, FacilitatorSettleResponse::Success(_))
    }

    pub fn success(success: FacilitatorSettleSuccess) -> Self {
        FacilitatorSettleResponse::Success(success)
    }

    pub fn failed(failed: FacilitatorSettleFailed) -> Self {
        FacilitatorSettleResponse::Failed(failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacilitatorSettleSuccess {
    pub payer: String,
    pub transaction: String,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacilitatorSettleFailed {
    pub error_reason: String,
    pub payer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilitatorSupportedKinds {
    pub x402_version: X402Version,
    pub scheme: String,
    pub network: String,
    pub extra: Option<AnyJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilitatorSupportedResponse {
    pub kinds: Vec<FacilitatorSupportedKinds>,
}

impl From<FacilitatorSettleSuccess> for PaymentResponse {
    fn from(success: FacilitatorSettleSuccess) -> Self {
        PaymentResponse {
            success: true,
            transaction: success.transaction,
            network: success.network,
            payer: success.payer,
        }
    }
}

/// X402 facilitator interface.
pub trait Facilitator {
    type Error: std::error::Error;

    fn supported(&self) -> impl Future<Output = Result<FacilitatorSupportedResponse, Self::Error>>;

    fn verify(
        &self,
        request: FacilitatorPaymentRequest,
    ) -> impl Future<Output = Result<FacilitatorVerifyResponse, Self::Error>>;

    fn settle(
        &self,
        request: FacilitatorPaymentRequest,
    ) -> impl Future<Output = Result<FacilitatorSettleResponse, Self::Error>>;
}
