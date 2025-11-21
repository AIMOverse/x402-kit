use std::fmt::Display;

use crypto_bigint::U256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmountValue(pub U256);

impl From<u8> for AmountValue {
    fn from(value: u8) -> Self {
        AmountValue(U256::from(value))
    }
}

impl From<u16> for AmountValue {
    fn from(value: u16) -> Self {
        AmountValue(U256::from(value))
    }
}

impl From<u32> for AmountValue {
    fn from(value: u32) -> Self {
        AmountValue(U256::from(value))
    }
}

impl From<u64> for AmountValue {
    fn from(value: u64) -> Self {
        AmountValue(U256::from(value))
    }
}

impl From<u128> for AmountValue {
    fn from(value: u128) -> Self {
        AmountValue(U256::from(value))
    }
}

impl Display for AmountValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for AmountValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for AmountValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let value = U256::from_str_radix_vartime(&s, 10).map_err(serde::de::Error::custom)?;
        Ok(AmountValue(value))
    }
}
