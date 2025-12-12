use serde::{Deserialize, Serialize};

use crate::{
    transport::{PaymentPayload, PaymentRequirements, SettlementResponse},
    types::{AnyJson, Base64EncodedHeader, Record, X402V2},
};

#[derive(Debug, Clone)]
pub struct PaymentRequest {
    pub payment_signature: Base64EncodedHeader,
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

#[derive(Debug, Clone)]
pub enum VerifyResponse {
    Valid(VerifyValid),
    Invalid(VerifyInvalid),
}

impl VerifyResponse {
    pub fn is_valid(&self) -> bool {
        matches!(self, VerifyResponse::Valid(_))
    }

    pub fn valid(valid: VerifyValid) -> Self {
        VerifyResponse::Valid(valid)
    }

    pub fn invalid(invalid: VerifyInvalid) -> Self {
        VerifyResponse::Invalid(invalid)
    }

    pub fn as_valid(&self) -> Option<&VerifyValid> {
        match self {
            VerifyResponse::Valid(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_invalid(&self) -> Option<&VerifyInvalid> {
        match self {
            VerifyResponse::Invalid(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifyValid {
    pub payer: String,
}

#[derive(Debug, Clone)]
pub struct VerifyInvalid {
    pub invalid_reason: String,
    pub payer: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SettleResponse {
    Success(SettleSuccess),
    Failed(SettleFailed),
}

impl SettleResponse {
    pub fn is_success(&self) -> bool {
        matches!(self, SettleResponse::Success(_))
    }

    pub fn success(success: SettleSuccess) -> Self {
        SettleResponse::Success(success)
    }

    pub fn failed(failed: SettleFailed) -> Self {
        SettleResponse::Failed(failed)
    }

    pub fn as_success(&self) -> Option<&SettleSuccess> {
        match self {
            SettleResponse::Success(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_failed(&self) -> Option<&SettleFailed> {
        match self {
            SettleResponse::Failed(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettleSuccess {
    pub payer: String,
    pub transaction: String,
    pub network: String,
}

#[derive(Debug, Clone)]
pub struct SettleFailed {
    pub error_reason: String,
    pub payer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedKinds {
    pub x402_version: X402V2,
    pub scheme: String,
    pub network: String,
    pub extra: Option<AnyJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedResponse {
    pub kinds: Vec<SupportedKinds>,

    // TODO: implement stronger typings for extensions
    /// Array of extension identifiers the facilitator has implemented
    pub extensions: Vec<AnyJson>,
    /// Map of CAIP-2 patterns (e.g., eip155:*) to public signer addresses
    pub signers: Record<Vec<String>>,
}

impl From<SettleSuccess> for SettlementResponse {
    fn from(success: SettleSuccess) -> Self {
        SettlementResponse {
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

    fn supported(&self) -> impl Future<Output = Result<SupportedResponse, Self::Error>>;

    fn verify(
        &self,
        request: PaymentRequest,
    ) -> impl Future<Output = Result<VerifyResponse, Self::Error>>;

    fn settle(
        &self,
        request: PaymentRequest,
    ) -> impl Future<Output = Result<SettleResponse, Self::Error>>;
}
