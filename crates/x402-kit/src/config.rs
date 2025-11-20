use bon::{Builder, bon};
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
    pub description: String,
    /// Optional MIME type of the resource.
    pub mime_type: String,
    /// Optional output schema for the payment payload.
    pub output_schema: Option<OutputSchema>,
}

/// Per transport configuration options.
#[derive(Builder)]
pub struct TransportConfig<A: Address> {
    /// The address to use for payments.
    pub pay_to: A,
    /// The asset for the payment
    pub asset: Asset<A>,
    /// The amount of the asset to pay, in smallest units.
    pub amount: AmountValue,
    /// Maximum timeout in seconds for the payment to be completed.
    pub max_timeout_seconds: u64,
    /// Optional resource configuration.
    pub resource: Resource,
}

#[bon]
impl<A: Address> TransportConfig<A> {
    #[builder]
    pub fn into_payment_requirements<S>(self, scheme: S, extra: Option<Any>) -> PaymentRequirements
    where
        S: Scheme<Network = A::Network>,
    {
        PaymentRequirements {
            scheme: scheme.scheme_name().to_string(),
            network: scheme.network().network_name().to_string(),
            max_amount_required: self.amount,
            resource: self.resource.url,
            description: self.resource.description,
            mime_type: self.resource.mime_type,
            pay_to: self.pay_to.to_string(),
            max_timeout_seconds: self.max_timeout_seconds,
            asset: self.asset.address.to_string(),
            output_schema: self.resource.output_schema,
            extra,
        }
    }
}
