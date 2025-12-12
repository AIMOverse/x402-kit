use bon::Builder;
use url::Url;

use crate::{
    concepts::{Address, Asset, Scheme},
    types::{AmountValue, AnyJson, OutputSchema},
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
    pub extra: Option<AnyJson>,
}
