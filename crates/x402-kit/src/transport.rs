use std::fmt::Display;

use base64::{Engine, prelude::BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::types::{AmountValue, Any, OutputSchema, Record, X402Version};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    /// Scheme name, defined in "schemes" protocol
    pub scheme: String,
    /// Network name, defined in "schemes" protocol
    pub network: String,
    /// Maximum amount required for the payment in smallest units
    pub max_amount_required: AmountValue,
    /// Resource URL to fetch payment details
    pub resource: Url,
    /// Description of the resource
    pub description: String,
    /// MIME type of the payment payload
    pub mime_type: String,
    /// Destination address or account to pay to
    pub pay_to: String,
    /// Maximum timeout in seconds for the payment to be completed
    pub max_timeout_seconds: u64,
    /// Asset address or identifier
    pub asset: String,
    /// Schema of the input / output payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<OutputSchema>,
    /// Extra fields for extensibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Any>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: X402Version,
    pub scheme: String,
    pub network: String,
    pub payload: Any,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirementsResponse {
    pub x402_version: X402Version,
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
}

#[derive(Debug, Clone)]
pub struct FacilitatorPaymentRequest {
    pub payload: FacilitatorPaymentRequestPayload,
    pub headers: FacilitatorPaymentRequestHeaders,
}

#[derive(Debug, Clone)]
pub struct FacilitatorPaymentRequestPayload {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

#[derive(Debug, Clone)]
pub struct FacilitatorPaymentRequestHeaders {
    pub payment_header: Base64EncodedHeader,
    pub extra_headers: Record<String>,
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

#[derive(Debug, Clone)]
pub struct FacilitatorVerifyValid {
    pub payer: String,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct FacilitatorSettleSuccess {
    pub payer: String,
    pub transaction: String,
    pub network: String,
}

#[derive(Debug, Clone)]
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
    pub extra: Option<Any>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilitatorSupportedResponse {
    pub kinds: Vec<FacilitatorSupportedKinds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResponse {
    pub success: bool,
    pub transaction: String,
    pub network: String,
    pub payer: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base64EncodedHeader(pub String);

impl Serialize for Base64EncodedHeader {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Base64EncodedHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Base64EncodedHeader(s))
    }
}

impl Display for Base64EncodedHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<PaymentPayload> for Base64EncodedHeader {
    type Error = serde_json::Error;

    fn try_from(value: PaymentPayload) -> Result<Self, Self::Error> {
        let json = serde_json::to_string(&value)?;
        let encoded = BASE64_STANDARD.encode(json);
        Ok(Base64EncodedHeader(encoded))
    }
}

impl TryFrom<Base64EncodedHeader> for PaymentPayload {
    type Error = crate::errors::Error;

    fn try_from(value: Base64EncodedHeader) -> Result<Self, Self::Error> {
        let decoded_bytes = BASE64_STANDARD.decode(&value.0)?;
        let json_str = String::from_utf8(decoded_bytes)?;
        let payload = serde_json::from_str(&json_str)?;
        Ok(payload)
    }
}

impl TryFrom<PaymentResponse> for Base64EncodedHeader {
    type Error = serde_json::Error;

    fn try_from(value: PaymentResponse) -> Result<Self, Self::Error> {
        let json = serde_json::to_string(&value)?;
        let encoded = BASE64_STANDARD.encode(json);
        Ok(Base64EncodedHeader(encoded))
    }
}

impl TryFrom<Base64EncodedHeader> for PaymentResponse {
    type Error = crate::errors::Error;

    fn try_from(value: Base64EncodedHeader) -> Result<Self, Self::Error> {
        let decoded_bytes = BASE64_STANDARD.decode(&value.0)?;
        let json_str = String::from_utf8(decoded_bytes)?;
        let response = serde_json::from_str(&json_str)?;
        Ok(response)
    }
}
