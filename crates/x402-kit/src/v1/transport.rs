use base64::{Engine, prelude::BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    core::{Address, NetworkFamily, Payment, Scheme},
    types::{AmountValue, AnyJson, Base64EncodedHeader, OutputSchema, X402Version},
};

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
    pub extra: Option<AnyJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: X402Version,
    pub scheme: String,
    pub network: String,
    pub payload: AnyJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirementsResponse {
    pub x402_version: X402Version,
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResponse {
    pub success: bool,
    pub transaction: String,
    pub network: String,
    pub payer: String,
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

impl<S, A> From<Payment<S, A>> for PaymentRequirements
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    fn from(payment: Payment<S, A>) -> Self {
        PaymentRequirements {
            scheme: S::SCHEME_NAME.to_string(),
            network: payment.scheme.network().network_name().to_string(),
            max_amount_required: payment.amount,
            resource: payment.resource.url,
            description: payment.resource.description,
            mime_type: payment.resource.mime_type,
            pay_to: payment.pay_to.to_string(),
            max_timeout_seconds: payment.max_timeout_seconds,
            asset: payment.asset.address.to_string(),
            output_schema: payment.resource.output_schema,
            extra: payment.extra,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;
    use serde_json::Value;

    use crate::{
        core::Resource,
        networks::evm::{
            EvmNetwork, ExplicitEvmAsset, ExplicitEvmNetwork, assets::UsdcBaseSepolia,
            networks::BaseSepolia,
        },
    };

    struct ExampleExactEvmScheme(EvmNetwork);

    impl Scheme for ExampleExactEvmScheme {
        type Network = EvmNetwork;
        type Payload = Value;
        const SCHEME_NAME: &'static str = "exact";

        fn network(&self) -> &Self::Network {
            &self.0
        }
    }

    use super::*;

    #[test]
    fn test_configure_payment_requirements() {
        let resource = Resource::builder()
            .url(Url::parse("https://example.com/payment").unwrap())
            .description("Payment for services".to_string())
            .mime_type("application/json".to_string())
            .build();

        let config = Payment::builder()
            .amount(1000u64)
            .asset(UsdcBaseSepolia)
            .max_timeout_seconds(300)
            .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
            .resource(resource)
            .scheme(ExampleExactEvmScheme(BaseSepolia::NETWORK))
            .build();

        let payment_requirements = PaymentRequirements::from(config);

        assert_eq!(payment_requirements.scheme, "exact");
        assert_eq!(payment_requirements.network, "base-sepolia");
        assert_eq!(payment_requirements.max_amount_required, 1000u64.into());
        assert_eq!(
            payment_requirements.resource,
            Url::parse("https://example.com/payment").unwrap()
        );
        assert_eq!(payment_requirements.description, "Payment for services");
        assert_eq!(payment_requirements.mime_type, "application/json");
        assert_eq!(
            payment_requirements.pay_to,
            "0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"
        );
        assert_eq!(payment_requirements.max_timeout_seconds, 300);
        assert_eq!(
            payment_requirements.asset,
            UsdcBaseSepolia::ASSET.address.to_string()
        );
    }
}
