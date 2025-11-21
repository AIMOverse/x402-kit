use bon::Builder;
use url::Url;

use crate::{
    concepts::{Address, Asset, NetworkFamily, Scheme},
    transport::PaymentRequirements,
    types::{AmountValue, Any, OutputSchema},
};

/// Resource configuration.
#[derive(Builder, Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// Optional resource URL.
    pub url: Url,
    /// Optional description of the resource.
    #[builder(into)]
    pub description: String,
    /// Optional MIME type of the resource.
    #[builder(into)]
    pub mime_type: String,
    /// Optional output schema for the payment payload.
    pub output_schema: Option<OutputSchema>,
}

/// Per transport configuration options.
#[derive(Builder, Debug, Clone)]
pub struct TransportConfig<A: Address> {
    /// The address to use for payments.
    #[builder(into)]
    pub pay_to: A,
    /// The asset for the payment
    #[builder(into)]
    pub asset: Asset<A>,
    /// The amount of the asset to pay, in smallest units.
    #[builder(into)]
    pub amount: AmountValue,
    /// Maximum timeout in seconds for the payment to be completed.
    pub max_timeout_seconds: u64,
    /// Optional resource configuration.
    pub resource: Resource,
}

/// Payment requirements configuration for a given scheme and transport.
#[derive(Builder, Debug, Clone)]
pub struct PaymentRequirementsConfig<S, A>
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    pub scheme: S,
    pub transport: TransportConfig<A>,
    pub extra: Option<Any>,
}

impl<S, A> From<PaymentRequirementsConfig<S, A>> for PaymentRequirements
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    fn from(config: PaymentRequirementsConfig<S, A>) -> Self {
        PaymentRequirements {
            scheme: config.scheme.scheme_name().to_string(),
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

    use crate::networks::evm::{
        EvmNetwork, ExplicitEvmAsset, ExplicitEvmNetwork, assets::UsdcBaseSepolia,
        networks::BaseSepolia,
    };

    struct ExampleExactEvmScheme(EvmNetwork);

    impl Scheme for ExampleExactEvmScheme {
        type Network = EvmNetwork;
        type Payload = Value;

        fn network(&self) -> &Self::Network {
            &self.0
        }

        fn scheme_name(&self) -> &str {
            "exact"
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
            .scheme(ExampleExactEvmScheme(BaseSepolia::network()))
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
            UsdcBaseSepolia::asset().address.to_string()
        );
    }
}
