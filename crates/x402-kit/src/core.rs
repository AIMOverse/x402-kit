//! Core traits and types used across the X402 Kit.

use std::{fmt::Display, str::FromStr};

use bon::Builder;
use url::Url;

use crate::types::{AmountValue, AnyJson, OutputSchema};

/// A series of network families, e.g. EVM, SVM, etc.
pub trait NetworkFamily {
    /// The name of the network in the family.
    fn network_name(&self) -> &str;

    /// The Blockchain network identifier in CAIP-2 format (e.g., "eip155:84532")
    fn network_id(&self) -> &str;
}

/// Network-specific address type.
pub trait Address: FromStr + Display + Copy {
    /// The network family this address belongs to.
    type Network: NetworkFamily;
}

/// A payment scheme applied to a network family.
pub trait Scheme {
    /// The network family this scheme applies to.
    type Network: NetworkFamily;
    /// The payload type produced by this scheme.
    type Payload;
    /// The name of the scheme.
    const SCHEME_NAME: &'static str;
    /// Get the concrete network for this scheme.
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

/// Payment configuration for a given scheme and transport.
#[derive(Builder)]
pub struct Payment<S, A>
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    /// The payment scheme.
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
    /// Optional extra data for the payment.
    pub extra: Option<AnyJson>,
    /// Resource definition.
    pub resource: Resource,
    /// Extensions
    #[builder(default)]
    pub extensions: AnyJson,
}

/// Payment configuration for a given scheme and transport.
#[derive(Builder)]
pub struct PaymentSelection<A: Address> {
    /// The address to use for payments.
    #[builder(into)]
    pub pay_to: A,
    /// The asset for the payment
    #[builder(into)]
    pub asset: A,
    /// The amount of the asset to pay, in smallest units.
    #[builder(into)]
    pub amount: AmountValue,
    /// Maximum timeout in seconds for the payment to be completed.
    pub max_timeout_seconds: u64,
    /// Optional extra data for the payment.
    pub extra: Option<AnyJson>,
    /// Resource definition.
    pub resource: Resource,
    /// Extensions
    #[builder(default)]
    pub extensions: AnyJson,
}

/// Signer for a given payment scheme.
pub trait SchemeSigner<A: Address<Network = <Self::Scheme as Scheme>::Network>> {
    type Scheme: Scheme;
    type Error: std::error::Error;

    fn sign(
        &self,
        payment: &PaymentSelection<A>,
    ) -> impl Future<Output = Result<<Self::Scheme as Scheme>::Payload, Self::Error>>;
}

/// Resource definition.
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
