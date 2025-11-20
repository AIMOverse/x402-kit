use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use solana_sdk::pubkey::{ParsePubkeyError, Pubkey};

use crate::concepts::{Address, NetworkFamily};

pub struct SvmNetwork(pub &'static str);

impl NetworkFamily for SvmNetwork {
    fn network_name(&self) -> &str {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SvmAddress(pub Pubkey);

impl From<Pubkey> for SvmAddress {
    fn from(pk: Pubkey) -> Self {
        SvmAddress(pk)
    }
}

impl FromStr for SvmAddress {
    type Err = ParsePubkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pk = Pubkey::from_str(s)?;
        Ok(SvmAddress(pk))
    }
}

impl Display for SvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl Debug for SvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SvmAddress({})", self.0)
    }
}

impl Address for SvmAddress {
    type Network = SvmNetwork;
}

pub type SvmAsset = crate::concepts::Asset<SvmAddress>;

pub mod networks {
    use super::*;

    pub const SOLANA: SvmNetwork = SvmNetwork("solana");
    pub const SOLANA_DEVNET: SvmNetwork = SvmNetwork("solana-devnet");
}

pub mod assets {
    use solana_sdk::pubkey;

    use super::*;

    pub fn usdc(network: &SvmNetwork) -> Option<SvmAsset> {
        fn create_usdc(pubkey: Pubkey) -> SvmAsset {
            SvmAsset {
                address: SvmAddress(pubkey),
                decimals: 6,
                name: "USD Coin",
                symbol: "USDC",
            }
        }
        match network.0 {
            "solana" => Some(create_usdc(pubkey!(
                "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
            ))),
            "solana-devnet" => Some(create_usdc(pubkey!(
                "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
            ))),
            _ => None,
        }
    }
}
