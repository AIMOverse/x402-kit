use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    concepts::Scheme,
    config::{PaymentRequirementsConfig, Resource, TransportConfig},
    networks::svm::{ExplicitSvmAsset, ExplicitSvmNetwork, SvmAddress, SvmNetwork},
    transport::PaymentRequirements,
};

#[derive(Builder, Debug, Clone)]
pub struct ExactSvm<A: ExplicitSvmAsset> {
    pub asset: A,
    #[builder(into)]
    pub pay_to: SvmAddress,
    pub amount: u64,
    pub max_timeout_seconds_override: Option<u64>,
    pub resource: Resource,
    #[builder(into)]
    pub fee_payer: SvmAddress,
}

impl<A: ExplicitSvmAsset> ExactSvm<A> {
    pub fn into_config(self) -> PaymentRequirementsConfig<ExactSvmScheme, SvmAddress> {
        PaymentRequirementsConfig {
            scheme: ExactSvmScheme(A::Network::NETWORK),
            transport: TransportConfig::builder()
                .amount(self.amount)
                .asset(A::ASSET)
                .pay_to(self.pay_to)
                .max_timeout_seconds(self.max_timeout_seconds_override.unwrap_or(60))
                .resource(self.resource)
                .build(),
            extra: Some(serde_json::json!({ "feePayer": self.fee_payer.to_string() })),
        }
    }

    pub fn into_payment_requirements(self) -> PaymentRequirements {
        self.into_config().into()
    }
}

pub struct ExactSvmScheme(pub SvmNetwork);

impl Scheme for ExactSvmScheme {
    type Network = SvmNetwork;
    type Payload = ExplicitSvmPayload;
    fn network(&self) -> &Self::Network {
        &self.0
    }
    fn scheme_name(&self) -> &str {
        "exact"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplicitSvmPayload {
    #[serde(rename = "transaction")]
    pub encoded_transaction: String,
}

#[cfg(test)]
mod tests {
    use solana_pubkey::pubkey;
    use url::Url;

    use crate::{
        config::Resource, networks::svm::assets::UsdcSolanaDevnet, schemes::exact_svm::ExactSvm,
    };

    #[test]
    fn test_build_payment_requirements() {
        let resource = Resource::builder()
            .url(Url::parse("https://example.com/payment").unwrap())
            .description("Payment for services".to_string())
            .mime_type("application/json".to_string())
            .build();
        let config = ExactSvm::builder()
            .asset(UsdcSolanaDevnet)
            .amount(1000)
            .fee_payer(pubkey!("JB63cv6eR67ntKaHQQufmgwDmxqZtX1ZwXgJCvcYKzC5"))
            .pay_to(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR"))
            .resource(resource)
            .build()
            .into_payment_requirements();

        assert_eq!(config.scheme, "exact");
        assert_eq!(config.network, "solana-devnet");
        assert_eq!(config.max_amount_required, 1000u64.into());
        assert_eq!(
            config.resource,
            Url::parse("https://example.com/payment").unwrap()
        );
        assert_eq!(
            config.extra,
            Some(serde_json::json!({
                "feePayer": "JB63cv6eR67ntKaHQQufmgwDmxqZtX1ZwXgJCvcYKzC5"
            }))
        );
    }
}
