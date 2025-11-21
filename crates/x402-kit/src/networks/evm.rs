use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::concepts::{Address, Asset, NetworkFamily, Signature};

#[derive(Debug, Clone, Copy)]
pub struct EvmNetwork {
    pub name: &'static str,
    pub chain_id: u64,
}

impl NetworkFamily for EvmNetwork {
    fn network_name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EvmAddress(pub alloy_primitives::Address);

impl From<alloy_primitives::Address> for EvmAddress {
    fn from(addr: alloy_primitives::Address) -> Self {
        EvmAddress(addr)
    }
}

impl FromStr for EvmAddress {
    type Err = alloy_primitives::AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let addr = alloy_primitives::Address::from_str(s)?;
        Ok(EvmAddress(addr))
    }
}

impl Display for EvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl Debug for EvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EvmAddress({})", self.0)
    }
}

impl Serialize for EvmAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EvmAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        EvmAddress::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Address for EvmAddress {
    type Network = EvmNetwork;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EvmSignature(pub alloy_primitives::Signature);

impl Display for EvmSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl Debug for EvmSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EvmSignature({})", self.0)
    }
}

impl FromStr for EvmSignature {
    type Err = alloy_primitives::SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sig = alloy_primitives::Signature::from_str(s)?;
        Ok(EvmSignature(sig))
    }
}

impl Serialize for EvmSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EvmSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        EvmSignature::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Signature for EvmSignature {
    type Network = EvmNetwork;
}

pub type EvmAsset = Asset<EvmAddress>;

pub mod networks {
    use super::*;

    pub const ETHEREUM: EvmNetwork = EvmNetwork {
        name: "ethereum",
        chain_id: 1,
    };
    pub const ETHEREUM_SEPOLIA: EvmNetwork = EvmNetwork {
        name: "ethereum-sepolia",
        chain_id: 11155111,
    };
    pub const BASE: EvmNetwork = EvmNetwork {
        name: "base",
        chain_id: 8453,
    };
    pub const BASE_SEPOLIA: EvmNetwork = EvmNetwork {
        name: "base-sepolia",
        chain_id: 84531,
    };
}

pub mod assets {
    use alloy_primitives::address;

    use super::*;

    pub const ETH: EvmAsset = EvmAsset {
        address: EvmAddress(alloy_primitives::Address::ZERO),
        decimals: 18,
        name: "Ether",
        symbol: "ETH",
    };

    macro_rules! create_usdc {
        ($addr:expr) => {
            EvmAsset {
                address: EvmAddress($addr),
                decimals: 6,
                name: "USD Coin",
                symbol: "USDC",
            }
        };
    }

    pub const USDC_ETHEREUM: EvmAsset =
        create_usdc!(address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
    pub const USDC_ETHEREUM_SEPOLIA: EvmAsset =
        create_usdc!(address!("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238"));
    pub const USDC_BASE: EvmAsset =
        create_usdc!(address!("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"));
    pub const USDC_BASE_SEPOLIA: EvmAsset =
        create_usdc!(address!("0x036CbD53842c5426634e7929541eC2318f3dCF7e"));
}
