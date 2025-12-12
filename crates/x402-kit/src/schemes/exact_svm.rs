use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    concepts::Scheme,
    config::{PaymentRequirementsConfig, Resource, TransportConfig},
    networks::svm::{ExplicitSvmAsset, ExplicitSvmNetwork, SvmAddress, SvmNetwork},
};

#[derive(Builder, Debug, Clone)]
pub struct ExactSvm<A: ExplicitSvmAsset> {
    pub asset: A,
    #[builder(into)]
    pub pay_to: SvmAddress,
    pub amount: u64,
    pub max_timeout_seconds_override: Option<u64>,
    pub resource: Resource,
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
            // Fee payer should be updated with facilitator's supported networks list
            extra: None,
        }
    }
}

pub struct ExactSvmScheme(pub SvmNetwork);

impl Scheme for ExactSvmScheme {
    type Network = SvmNetwork;
    type Payload = ExplicitSvmPayload;
    const SCHEME_NAME: &'static str = "exact";

    fn network(&self) -> &Self::Network {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplicitSvmPayload {
    pub transaction: String,
}

#[cfg(test)]
mod tests {
    use solana_pubkey::pubkey;
    use url::Url;

    use crate::{
        config::Resource, networks::svm::assets::UsdcSolanaDevnet, schemes::exact_svm::ExactSvm,
        v1::transport::PaymentRequirements,
    };

    #[test]
    fn test_build_payment_requirements() {
        let resource = Resource::builder()
            .url(Url::parse("https://example.com/payment").unwrap())
            .description("Payment for services".to_string())
            .mime_type("application/json".to_string())
            .build();
        let pr: PaymentRequirements = ExactSvm::builder()
            .asset(UsdcSolanaDevnet)
            .amount(1000)
            .pay_to(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR"))
            .resource(resource)
            .build()
            .into_config()
            .into();

        assert_eq!(pr.scheme, "exact");
        assert_eq!(pr.network, "solana-devnet");
        assert_eq!(pr.max_amount_required, 1000u64.into());
        assert_eq!(
            pr.resource,
            Url::parse("https://example.com/payment").unwrap()
        );
        assert!(pr.extra.is_none());
    }
}
