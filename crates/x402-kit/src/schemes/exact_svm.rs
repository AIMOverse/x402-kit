use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    core::{Payment, Resource, Scheme},
    networks::svm::{ExplicitSvmAsset, ExplicitSvmNetwork, SvmAddress, SvmNetwork},
    types::Record,
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

impl<A: ExplicitSvmAsset> From<ExactSvm<A>> for Payment<ExactSvmScheme, SvmAddress> {
    fn from(scheme: ExactSvm<A>) -> Self {
        Payment {
            scheme: ExactSvmScheme(A::Network::NETWORK),
            pay_to: scheme.pay_to,
            asset: A::ASSET,
            amount: scheme.amount.into(),
            max_timeout_seconds: scheme.max_timeout_seconds_override.unwrap_or(60),
            resource: scheme.resource,
            extra: None,
            extensions: Record::new(),
        }
    }
}

impl<A: ExplicitSvmAsset> ExactSvm<A> {
    #[cfg(feature = "v1")]
    pub fn v1(self) -> crate::v1::transport::PaymentRequirements {
        crate::v1::transport::PaymentRequirements::from(Payment::from(self))
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
        core::Resource, networks::svm::assets::UsdcSolanaDevnet, schemes::exact_svm::ExactSvm,
    };

    #[test]
    fn test_build_payment_requirements() {
        let resource = Resource::builder()
            .url(Url::parse("https://example.com/payment").unwrap())
            .description("Payment for services".to_string())
            .mime_type("application/json".to_string())
            .build();
        let pr: crate::v1::transport::PaymentRequirements = ExactSvm::builder()
            .asset(UsdcSolanaDevnet)
            .amount(1000)
            .pay_to(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR"))
            .resource(resource)
            .build()
            .v1();

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
