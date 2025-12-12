//! Core traits and types used across the X402 Kit.

use std::{fmt::Display, str::FromStr};

use bon::Builder;
use url::Url;

use crate::types::{AmountValue, AnyJson, OutputSchema};

/// A series of network families, e.g. EVM, SVM, etc.
pub trait NetworkFamily {
    /// The name of the network in the family.
    fn network_name(&self) -> &str;
}

/// Network-specific address type.
pub trait Address: FromStr + Display + Copy {
    type Network: NetworkFamily;
}

/// A payment scheme applied to a network family.
pub trait Scheme {
    type Network: NetworkFamily;
    type Payload;
    const SCHEME_NAME: &'static str;

    fn network(&self) -> &Self::Network;

    // fn select<A: Address<Network = Self::Network>>(
    //     &self,
    //     pr: &PaymentRequirements,
    // ) -> Option<PaymentSelection<A>>
    // where
    //     Self: Sized,
    // {
    //     if pr.scheme == Self::SCHEME_NAME && pr.network == self.network().network_name() {
    //         Some(PaymentSelection {
    //             max_amount_required: pr.max_amount_required,
    //             resource: pr.resource.clone(),
    //             description: pr.description.clone(),
    //             mime_type: pr.mime_type.clone(),
    //             pay_to: pr.pay_to.parse().ok()?,
    //             max_timeout_seconds: pr.max_timeout_seconds,
    //             asset: pr.asset.parse().ok()?,
    //             output_schema: pr.output_schema.clone(),
    //             extra: pr.extra.clone(),
    //         })
    //     } else {
    //         None
    //     }
    // }
}

/// Represents an asset on a given address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset<A: Address> {
    pub address: A,
    pub decimals: u8,
    pub name: &'static str,
    pub symbol: &'static str,
}

/// Payment requirements configuration for a given scheme and transport.
#[derive(Builder, Debug, Clone)]
pub struct Payment<S, A>
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    pub scheme: S,
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
    pub extra: Option<AnyJson>,
    /// Resource configuration.
    pub resource: Resource,
}

/// Signer for a given payment scheme.
pub trait SchemeSigner<A: Address<Network = <Self::Scheme as Scheme>::Network>> {
    type Scheme: Scheme;
    type Error: std::error::Error;

    fn sign(
        &self,
        selected: &Payment<Self::Scheme, A>,
    ) -> impl Future<Output = Result<<Self::Scheme as Scheme>::Payload, Self::Error>>;
}

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
