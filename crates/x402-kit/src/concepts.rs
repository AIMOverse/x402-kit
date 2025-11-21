//! Core traits and types used across the X402 Kit.

use std::{fmt::Display, marker::PhantomData, str::FromStr};

use url::Url;

use crate::{
    transport::{
        Base64EncodedPayload, FacilitatorPaymentRequest, FacilitatorSettleResponse,
        FacilitatorSupportedResponse, FacilitatorVerifyResponse, PaymentRequirements,
    },
    types::{AmountValue, Any, OutputSchema},
};

/// A series of network families, e.g. EVM, SVM, etc.
pub trait NetworkFamily {
    /// The name of the network in the family.
    fn network_name(&self) -> &str;
}

/// Network-specific address type.
pub trait Address: FromStr + Display + Copy {
    type Network: NetworkFamily;
}

pub trait Signature: FromStr + Display + Copy {
    type Network: NetworkFamily;
}

/// A payment scheme applied to a network family.
pub trait Scheme {
    type Network: NetworkFamily;
    type Payload;

    fn scheme_name(&self) -> &str;
    fn network(&self) -> &Self::Network;

    fn select_payment_requirements<A: Address<Network = Self::Network>>(
        &self,
        pr: &PaymentRequirements,
    ) -> Option<SelectedPaymentRequirements<Self, A>>
    where
        Self: Sized,
    {
        if pr.scheme == self.scheme_name() && pr.network == self.network().network_name() {
            Some(SelectedPaymentRequirements {
                scheme: PhantomData,
                max_amount_required: pr.max_amount_required,
                resource: pr.resource.clone(),
                description: pr.description.clone(),
                mime_type: pr.mime_type.clone(),
                pay_to: pr.pay_to.parse().ok()?,
                max_timeout_seconds: pr.max_timeout_seconds,
                asset: pr.asset.parse().ok()?,
                output_schema: pr.output_schema.clone(),
                extra: pr.extra.clone(),
            })
        } else {
            None
        }
    }
}

/// Represents an asset on a given address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset<A: Address> {
    pub address: A,
    pub decimals: u8,
    pub name: &'static str,
    pub symbol: &'static str,
}

/// Selected payment requirements for a given scheme and address type.
#[derive(Debug, Clone)]
pub struct SelectedPaymentRequirements<S, A>
where
    S: Scheme,
    A: Address<Network = S::Network>,
{
    pub scheme: PhantomData<S>,
    /// Maximum amount required for the payment in smallest units
    pub max_amount_required: AmountValue,
    /// Resource URL to fetch payment details
    pub resource: Url,
    /// Description of the resource
    pub description: String,
    /// MIME type of the payment payload
    pub mime_type: String,
    /// Destination address or account to pay to
    pub pay_to: A,
    /// Maximum timeout in seconds for the payment to be completed
    pub max_timeout_seconds: u64,
    /// Asset address or identifier
    pub asset: A,
    /// Schema of the input / output payload
    pub output_schema: Option<OutputSchema>,
    /// Extra fields for extensibility
    pub extra: Option<Any>,
}

/// Signer for a given payment scheme.
pub trait SchemeSigner {
    type Scheme: Scheme;
    type Error: std::error::Error;

    fn sign<A: Address<Network = <Self::Scheme as Scheme>::Network>>(
        &self,
        selected: &SelectedPaymentRequirements<Self::Scheme, A>,
    ) -> impl Future<Output = Result<<Self::Scheme as Scheme>::Payload, Self::Error>>;
}

/// X402 facilitator interface.
pub trait Facilitator {
    type Error: std::error::Error;

    fn supported(&self) -> impl Future<Output = Result<FacilitatorSupportedResponse, Self::Error>>;

    fn verify(
        &self,
        request: &FacilitatorPaymentRequest,
        payment_header: &Base64EncodedPayload,
    ) -> impl Future<Output = Result<FacilitatorVerifyResponse, Self::Error>>;

    fn settle(
        &self,
        request: &FacilitatorPaymentRequest,
        payment_header: &Base64EncodedPayload,
    ) -> impl Future<Output = Result<FacilitatorSettleResponse, Self::Error>>;
}
