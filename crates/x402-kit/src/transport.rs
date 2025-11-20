use serde::{Deserialize, Serialize};
use url::Url;

use crate::types::{AmountValue, Any, OutputSchema, X402Version};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: X402Version,
    pub scheme: String,
    pub network: String,
    pub payload: Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirementsResponse {
    pub x402_version: X402Version,
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
}
