use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};

pub type Record<V> = std::collections::HashMap<String, V>;

pub type AnyJson = serde_json::Value;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum X402Version {
    V1,
    V2,
}

impl Serialize for X402Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            X402Version::V1 => serializer.serialize_i8(1),
            X402Version::V2 => serializer.serialize_i8(2),
        }
    }
}

impl<'de> Deserialize<'de> for X402Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = i8::deserialize(deserializer)?;
        match v {
            1 => Ok(X402Version::V1),
            2 => Ok(X402Version::V2),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown X402 version: {}",
                v
            ))),
        }
    }
}

impl Display for X402Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            X402Version::V1 => write!(f, "1"),
            X402Version::V2 => write!(f, "2"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base64EncodedHeader(pub String);

impl Serialize for Base64EncodedHeader {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Base64EncodedHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Base64EncodedHeader(s))
    }
}

impl Display for Base64EncodedHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    pub info: AnyJson,
    pub schema: AnyJson,
}
