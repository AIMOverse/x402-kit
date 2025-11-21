use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    concepts::Scheme,
    config::{PaymentRequirementsConfig, Resource, TransportConfig},
    networks::evm::{EvmAddress, EvmNetwork, EvmSignature, ExplicitEvmAsset, ExplicitEvmNetwork},
    transports::PaymentRequirements,
    types::{AmountValue, Any},
};

use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    str::FromStr,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nonce([u8; 32]);

impl Debug for Nonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Nonce({})", hex::encode(self.0))
    }
}

impl Display for Nonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for Nonce {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Nonce(arr))
    }
}

impl Serialize for Nonce {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Nonce {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let nonce = Nonce::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(nonce)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimestampSeconds(pub i64);

impl Display for TimestampSeconds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for TimestampSeconds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TimeSeconds({})", self.0)
    }
}

impl Serialize for TimestampSeconds {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TimestampSeconds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let seconds = s.parse::<i64>().map_err(serde::de::Error::custom)?;
        Ok(TimestampSeconds(seconds))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactEvmPayload {
    pub signature: EvmSignature,
    pub authorization: ExactEvmAuthorization,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactEvmAuthorization {
    pub from: EvmAddress,
    pub to: EvmAddress,
    pub value: AmountValue,
    pub valid_after: TimestampSeconds,
    pub valid_before: TimestampSeconds,
    pub nonce: Nonce,
}

#[derive(Debug, Clone)]
pub struct ExactEvmScheme {
    pub network: EvmNetwork,
}

impl ExactEvmScheme {
    pub fn with_network(network: EvmNetwork) -> Self {
        ExactEvmScheme { network }
    }
}

impl Scheme for ExactEvmScheme {
    type Network = EvmNetwork;
    type Payload = ExactEvmPayload;

    fn network(&self) -> &Self::Network {
        &self.network
    }

    fn scheme_name(&self) -> &str {
        "exact"
    }
}

#[derive(Builder, Debug, Clone)]
pub struct ExactEvmConfig<N: ExplicitEvmNetwork, A: ExplicitEvmAsset<NETWORK = N>> {
    #[builder(default)]
    pub phantom: PhantomData<N>,
    pub asset: A,
    #[builder(into)]
    pub pay_to: EvmAddress,
    pub amount: u64,
    pub max_timeout_seconds_override: Option<u64>,
    pub resource: Resource,
    pub extra: Option<Any>,
}

impl<N, A> ExactEvmConfig<N, A>
where
    N: ExplicitEvmNetwork,
    A: ExplicitEvmAsset<NETWORK = N>,
{
    pub fn into_config(self) -> PaymentRequirementsConfig<ExactEvmScheme, EvmAddress> {
        PaymentRequirementsConfig {
            scheme: ExactEvmScheme::with_network(N::network()),
            transport: TransportConfig {
                pay_to: self.pay_to,
                asset: A::asset(),
                amount: self.amount.into(),
                max_timeout_seconds: self.max_timeout_seconds_override.unwrap_or(300),
                resource: self.resource,
            },
            extra: self.extra,
        }
    }

    pub fn into_payment_requirements(self) -> PaymentRequirements {
        self.into_config().into()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;
    use url::Url;

    use crate::networks::evm::assets::UsdcBaseSepolia;

    use super::*;

    #[test]
    fn test_build_payment_requirements() {
        let resource = Resource::builder()
            .url(Url::parse("https://example.com/payment").unwrap())
            .description("Payment for services".to_string())
            .mime_type("application/json".to_string())
            .build();
        let config = ExactEvmConfig::builder()
            .asset(UsdcBaseSepolia)
            .amount(1000)
            .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
            .resource(resource)
            .build();
        let payment_requirements = config.into_payment_requirements();

        assert_eq!(payment_requirements.scheme, "exact");
        assert_eq!(
            payment_requirements.asset,
            UsdcBaseSepolia::asset().address.to_string()
        );
        assert_eq!(payment_requirements.max_amount_required, 1000u64.into());
    }
}
