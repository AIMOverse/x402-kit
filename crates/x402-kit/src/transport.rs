use base64::{Engine, prelude::BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::types::{AmountValue, AnyJson, Base64EncodedHeader, X402Version};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub amount: AmountValue,
    pub asset: String,
    pub pay_to: String,
    pub max_timeout_seconds: u64,
    pub extra: Option<AnyJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResource {
    pub url: Url,
    pub description: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: X402Version,
    pub error: String,
    pub resource: PaymentResource,
    pub accepts: Vec<PaymentRequirements>,
    pub extensions: AnyJson,
}

impl TryFrom<PaymentRequired> for Base64EncodedHeader {
    type Error = crate::errors::Error;

    /// Serialize PaymentRequired into `PAYMENT-REQUIRED` header format
    fn try_from(value: PaymentRequired) -> Result<Self, Self::Error> {
        let json = serde_json::to_string(&value)?;
        let encoded = BASE64_STANDARD.encode(json);
        Ok(Base64EncodedHeader(encoded))
    }
}

impl TryFrom<Base64EncodedHeader> for PaymentRequired {
    type Error = crate::errors::Error;

    /// Deserialize `PAYMENT-REQUIRED` header into PaymentRequired
    fn try_from(value: Base64EncodedHeader) -> Result<Self, Self::Error> {
        let decoded = BASE64_STANDARD.decode(&value.0)?;
        let json_str = String::from_utf8(decoded)?;
        let payment_required: PaymentRequired = serde_json::from_str(&json_str)?;
        Ok(payment_required)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: X402Version,
    pub resource: PaymentResource,
    pub accepted: PaymentRequirements,
    pub payload: AnyJson,
    pub extensions: AnyJson,
}

impl TryFrom<PaymentPayload> for Base64EncodedHeader {
    type Error = crate::errors::Error;

    /// Serialize PaymentPayload into `PAYMENT-SIGNATURE` header format
    fn try_from(value: PaymentPayload) -> Result<Self, Self::Error> {
        let json = serde_json::to_string(&value)?;
        let encoded = BASE64_STANDARD.encode(json);
        Ok(Base64EncodedHeader(encoded))
    }
}

impl TryFrom<Base64EncodedHeader> for PaymentPayload {
    type Error = crate::errors::Error;

    /// Deserialize `PAYMENT-SIGNATURE` header into PaymentPayload
    fn try_from(value: Base64EncodedHeader) -> Result<Self, Self::Error> {
        let decoded_bytes = BASE64_STANDARD.decode(&value.0)?;
        let json_str = String::from_utf8(decoded_bytes)?;
        let payload = serde_json::from_str(&json_str)?;
        Ok(payload)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementResponse {
    pub success: bool,
    pub transaction: String,
    pub network: String,
    pub payer: String,
}

impl TryFrom<SettlementResponse> for Base64EncodedHeader {
    type Error = crate::errors::Error;

    /// Serialize SettlementResponse into `PAYMENT-RESPONSE` header format
    fn try_from(value: SettlementResponse) -> Result<Self, Self::Error> {
        let json = serde_json::to_string(&value)?;
        let encoded = BASE64_STANDARD.encode(json);
        Ok(Base64EncodedHeader(encoded))
    }
}

impl TryFrom<Base64EncodedHeader> for SettlementResponse {
    type Error = crate::errors::Error;

    /// Deserialize `PAYMENT-RESPONSE` header into SettlementResponse
    fn try_from(value: Base64EncodedHeader) -> Result<Self, Self::Error> {
        let decoded_bytes = BASE64_STANDARD.decode(&value.0)?;
        let json_str = String::from_utf8(decoded_bytes)?;
        let response = serde_json::from_str(&json_str)?;
        Ok(response)
    }
}
