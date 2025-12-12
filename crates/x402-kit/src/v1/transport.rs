use std::fmt::Display;

use base64::{Engine, prelude::BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    concepts::{Address, NetworkFamily, Scheme},
    config::PaymentRequirementsConfig,
    types::{AmountValue, AnyJson, OutputSchema, X402Version},
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

impl<S, A> From<PaymentRequirementsConfig<S, A>> for PaymentRequirements
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    fn from(config: PaymentRequirementsConfig<S, A>) -> Self {
        PaymentRequirements {
            scheme: S::SCHEME_NAME.to_string(),
            network: config.scheme.network().network_name().to_string(),
            max_amount_required: config.transport.amount,
            resource: config.transport.resource.url,
            description: config.transport.resource.description,
            mime_type: config.transport.resource.mime_type,
            pay_to: config.transport.pay_to.to_string(),
            max_timeout_seconds: config.transport.max_timeout_seconds,
            asset: config.transport.asset.address.to_string(),
            output_schema: config.transport.resource.output_schema,
            extra: config.extra,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;
    use serde_json::Value;

    use crate::{
        config::{Resource, TransportConfig},
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

        let config = PaymentRequirementsConfig::builder()
            .transport(
                TransportConfig::builder()
                    .amount(1000u64)
                    .asset(UsdcBaseSepolia)
                    .max_timeout_seconds(300)
                    .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
                    .resource(resource)
                    .build(),
            )
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
