//! Core traits used across the X402 Kit.

use std::{fmt::Display, str::FromStr};

/// A series of network families, e.g. EVM, SVM, etc.
pub trait NetworkFamily {
    /// The name of the network in the family.
    fn network_name(&self) -> &str;
}

/// Network-specific address type.
pub trait Address: FromStr + Display + Copy + PartialEq + Eq {
    type Network: NetworkFamily;
}

/// A payment scheme applied to a network family.
pub trait Scheme {
    type Network: NetworkFamily;

    fn scheme_name(&self) -> &str;
    fn network(&self) -> &Self::Network;
}

/// Represents an asset on a given address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset<A: Address> {
    pub address: A,
    pub decimals: u8,
    pub name: &'static str,
    pub symbol: &'static str,
}
