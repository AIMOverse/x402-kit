use alloy_core::{
    sol,
    sol_types::{Eip712Domain, SolStruct, eip712_domain},
};
use alloy_primitives::{FixedBytes, U256};
use alloy_signer::{Error as AlloySignerError, Signer as AlloySigner};
use serde::Deserialize;

use crate::{
    core::{PaymentSelection, Scheme, SchemeSigner},
    networks::evm::{EvmAddress, EvmSignature, ExplicitEvmAsset, ExplicitEvmNetwork},
    schemes::exact_evm::*,
};

use std::{fmt::Debug, time::SystemTime};

pub trait AuthorizationSigner {
    type Error: std::error::Error;

    fn sign_authorization(
        &self,
        authorization: &Eip3009Authorization,
        asset_eip712_domain: &Eip712Domain,
    ) -> impl Future<Output = Result<EvmSignature, Self::Error>>;
}

sol!(
    /// Represent EIP-3009 Authorization struct
    ///
    /// For generating the EIP-712 signing hash
    struct Eip3009Authorization {
        address from;
        address to;
        uint256 value;
        uint256 validAfter;
        uint256 validBefore;
        bytes32 nonce;
    }
);

impl From<ExactEvmAuthorization> for Eip3009Authorization {
    fn from(authorization: ExactEvmAuthorization) -> Self {
        Eip3009Authorization {
            from: authorization.from.0,
            to: authorization.to.0,
            value: U256::from(authorization.value.0),
            validAfter: U256::from(authorization.valid_after.0),
            validBefore: U256::from(authorization.valid_before.0),
            nonce: FixedBytes(authorization.nonce.0),
        }
    }
}

impl<S: AlloySigner> AuthorizationSigner for S {
    type Error = AlloySignerError;

    async fn sign_authorization(
        &self,
        authorization: &Eip3009Authorization,
        domain: &Eip712Domain,
    ) -> Result<EvmSignature, Self::Error> {
        let eip712_hash = authorization.eip712_signing_hash(domain);
        let signature = self.sign_hash(&eip712_hash).await?;

        Ok(EvmSignature(signature))
    }
}

pub struct ExactEvmSigner<S: AuthorizationSigner, A: ExplicitEvmAsset> {
    pub signer: S,
    pub asset: A,
}

#[derive(Debug, thiserror::Error)]
pub enum ExactEvmSignError<S: AuthorizationSigner> {
    #[error("Signer error: {0}")]
    SignerError(S::Error),
    #[error("System time error: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
}

impl<S, A> SchemeSigner<EvmAddress> for ExactEvmSigner<S, A>
where
    S: AuthorizationSigner + Debug,
    A: ExplicitEvmAsset,
{
    type Scheme = ExactEvmScheme;
    type Error = ExactEvmSignError<S>;

    async fn sign(
        &self,
        selected: &PaymentSelection<EvmAddress>,
    ) -> Result<<Self::Scheme as Scheme>::Payload, Self::Error> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        #[derive(Deserialize, Default)]
        struct Eip712DomainExtra {
            name: String,
            version: String,
        }

        let eip712_domain_info = selected
            .extra
            .as_ref()
            .and_then(|extra| serde_json::from_value::<Eip712DomainExtra>(extra.clone()).ok())
            // Use empty string if not provided -- This doesn't work in many cases!
            .unwrap_or_default();

        let authorization = ExactEvmAuthorization {
            from: selected.pay_to,
            to: selected.pay_to,
            value: selected.amount,
            // Valid after: now - 5mins
            valid_after: TimestampSeconds(now.saturating_sub(300)),
            valid_before: TimestampSeconds(now + selected.max_timeout_seconds),
            nonce: Nonce(rand::random()),
        };

        let signer = &self.signer;
        let auth_clone = authorization.clone();
        let domain = eip712_domain!(
            name: eip712_domain_info.name,
            version: eip712_domain_info.version,
            chain_id: A::Network::NETWORK.chain_id,
            verifying_contract: A::ASSET.address.0,
        );
        let signature = signer
            .sign_authorization(&auth_clone.into(), &domain)
            .await
            .map_err(Self::Error::SignerError)?;
        Ok(ExactEvmPayload {
            signature,
            authorization,
        })
    }
}

#[cfg(test)]
mod tests {
    use alloy::signers::local::PrivateKeySigner;
    use alloy_primitives::address;
    use serde_json::json;
    use url::Url;

    use crate::{
        core::Resource,
        networks::evm::{assets::UsdcBaseSepolia, networks::BaseSepolia},
        types::{AmountValue, Record},
    };

    use super::*;

    #[tokio::test]
    async fn test_signing() {
        let signer = PrivateKeySigner::random();

        let evm_signer = ExactEvmSigner {
            signer,
            asset: UsdcBaseSepolia,
        };

        let resource = Resource::builder()
            .url(Url::parse("https://example.com/payment").unwrap())
            .description("Payment for services".to_string())
            .mime_type("application/json".to_string())
            .build();

        let payment = PaymentSelection {
            amount: 1000u64.into(),
            resource,
            pay_to: EvmAddress(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20")),
            max_timeout_seconds: 60,
            asset: UsdcBaseSepolia::ASSET.address,
            extra: Some(json!({
                "name": "USD Coin",
                "version": "2"
            })),
            extensions: Record::new(),
        };

        let payload = evm_signer
            .sign(&payment)
            .await
            .expect("Signing should succeed");

        assert_eq!(payload.authorization.value, AmountValue(1000));

        // Verify the signature
        let domain = eip712_domain! {
            name: "USD Coin".to_string(),
            version: "2".to_string(),
            chain_id: BaseSepolia::NETWORK.chain_id,
            verifying_contract: UsdcBaseSepolia::ASSET.address.0,
        };

        let recovered_address = payload
            .signature
            .0
            .recover_address_from_prehash(
                &Eip3009Authorization::from(payload.authorization.clone())
                    .eip712_signing_hash(&domain.into()),
            )
            .expect("Recovery should succeed");

        assert_eq!(recovered_address, evm_signer.signer.address());
    }
}
