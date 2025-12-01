//! SVM (Solana Virtual Machine) signer for the exact payment scheme.
//!
//! This module provides the [`ExactSvmSigner`] type for signing Solana transactions
//! to fulfill X402 payment requirements. It follows the same pattern as the TypeScript
//! client implementation.

use std::fmt::Debug;

use base64::prelude::*;
use bon::Builder;
use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;

use crate::{
    concepts::{PaymentSelection, Scheme, SchemeSigner},
    networks::svm::{ExplicitSvmAsset, SvmAddress},
    schemes::exact_svm::*,
};

/// Blanket implementation for any type that implements the Solana Signer trait.
impl<T: solana_signer::Signer> TransactionSigner for T {
    type Error = solana_signer::SignerError;

    async fn pubkey(&self) -> Pubkey {
        solana_signer::Signer::pubkey(self)
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Signature, Self::Error> {
        Ok(solana_signer::Signer::sign_message(self, message))
    }
}

pub trait TransactionSigner {
    type Error: std::error::Error;

    /// Returns the public key of the signer.
    fn pubkey(&self) -> impl Future<Output = Pubkey>;

    /// Signs a message and returns the signature.
    fn sign_message(&self, message: &[u8]) -> impl Future<Output = Result<Signature, Self::Error>>;

    /// Signs a transaction in place.
    fn sign_transaction(
        &self,
        transaction: &mut Transaction,
    ) -> impl Future<Output = Result<(), Self::Error>> {
        async {
            let message_data = transaction.message_data();
            let signature = self.sign_message(&message_data).await?;

            // Find the position of the signer in the transaction
            let pubkey = self.pubkey().await;
            let position = transaction
                .message
                .account_keys
                .iter()
                .position(|key| key == &pubkey);

            if let Some(pos) = position {
                if pos < transaction.signatures.len() {
                    transaction.signatures[pos] = signature;
                }
            }

            Ok(())
        }
    }
}

/// Exact SVM signer for creating and signing Solana transfer transactions.
///
/// This signer creates SPL token transfer transactions following the X402
/// exact payment scheme specification.
#[derive(Builder, Debug)]
pub struct ExactSvmSigner<S: TransactionSigner, A: ExplicitSvmAsset> {
    /// The underlying transaction signer.
    pub signer: S,
    /// The asset being transferred.
    pub asset: A,
    /// Blockhash to use.
    pub blockhash: Hash,
    /// Compute unit limit for the transaction.
    pub compute_unit_limit: u32,
    /// Compute unit price (priority fee) in micro-lamports.
    pub compute_unit_price: u64,
}

/// Error type for SVM signing operations.
#[derive(Debug, thiserror::Error)]
pub enum ExactSvmSignError<S: TransactionSigner> {
    #[error("Signer error: {0}")]
    SignerError(S::Error),
    #[error("Base64 encoding error: {0}")]
    Base64Error(#[from] base64::EncodeSliceError),
    #[error("Bincode serialization error: {0}")]
    BincodeError(#[from] bincode::error::EncodeError),
    #[error("Missing fee payer in payment requirements extra field")]
    MissingFeePayer,
    #[error("Invalid fee payer address: {0}")]
    InvalidFeePayer(String),
}

impl<S, A> ExactSvmSigner<S, A>
where
    S: TransactionSigner + Debug,
    A: ExplicitSvmAsset,
{
    /// Creates transfer instructions for the payment.
    ///
    /// This generates the SPL token transfer instruction with the proper
    /// source and destination associated token accounts.
    async fn create_transfer_instructions(
        &self,
        selected: &PaymentSelection<SvmAddress>,
    ) -> Vec<Instruction> {
        let pubkey = self.signer.pubkey().await;
        let source_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pubkey,
            &A::ASSET.address.0,
            &spl_token_2022::ID,
        );

        let destination_ata =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &selected.pay_to.0,
                &A::ASSET.address.0,
                &spl_token_2022::ID,
            );

        // Create transfer checked instruction
        // Convert u128 to u64 for SPL token (Solana amounts are u64)
        let amount = selected
            .max_amount_required
            .0
            .try_into()
            .unwrap_or(u64::MAX);
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &spl_token_2022::ID,
            &source_ata,
            &A::ASSET.address.0,
            &destination_ata,
            &pubkey,
            &[],
            amount,
            A::ASSET.decimals,
        )
        .expect("Failed to create transfer instruction");

        vec![transfer_ix]
    }

    /// Creates compute budget instructions for the transaction.
    fn create_compute_budget_instructions(&self) -> Vec<Instruction> {
        let mut instructions = Vec::new();

        // Set compute unit limit if specified
        instructions.push(
            solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
                self.compute_unit_limit,
            )
            .into(),
        );

        // Set compute unit price if specified
        instructions.push(
            solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_price(
                self.compute_unit_price,
            )
            .into(),
        );

        instructions
    }

    /// Creates and signs a payment transaction.
    ///
    /// # Arguments
    /// * `selected` - The payment selection with requirements
    /// * `config` - Optional signer configuration
    ///
    /// # Returns
    /// The signed transaction payload ready for submission.
    pub async fn sign_selected_payment(
        &self,
        selected: &PaymentSelection<SvmAddress>,
    ) -> Result<ExplicitSvmPayload, ExactSvmSignError<S>> {
        let pubkey = self.signer.pubkey().await;
        // Get fee payer from extra field or use signer's pubkey
        let fee_payer = if let Some(extra) = &selected.extra {
            extra
                .get("feePayer")
                .and_then(|v| v.as_str())
                .map(|s| {
                    s.parse::<Pubkey>()
                        .map_err(|_| ExactSvmSignError::InvalidFeePayer(s.to_string()))
                })
                .transpose()?
                .unwrap_or(pubkey)
        } else {
            pubkey
        };

        // Build instructions
        let mut instructions = self.create_compute_budget_instructions();
        instructions.extend(self.create_transfer_instructions(selected).await);

        let blockhash = self.blockhash;

        // Create the transaction message
        let message = Message::new_with_blockhash(&instructions, Some(&fee_payer), &blockhash);

        // Create transaction with placeholder signatures
        let mut transaction = Transaction::new_unsigned(message);

        // Sign the transaction
        self.signer
            .sign_transaction(&mut transaction)
            .await
            .map_err(ExactSvmSignError::SignerError)?;

        // Serialize and encode the transaction
        let serialized = bincode::serde::encode_to_vec(&transaction, bincode::config::legacy())?;
        let encoded = BASE64_STANDARD.encode(&serialized);

        Ok(ExplicitSvmPayload {
            transaction: encoded,
        })
    }
}

impl<S, A> SchemeSigner<SvmAddress> for ExactSvmSigner<S, A>
where
    S: TransactionSigner + Debug,
    A: ExplicitSvmAsset,
{
    type Scheme = ExactSvmScheme;
    type Error = ExactSvmSignError<S>;

    async fn sign(
        &self,
        selected: &PaymentSelection<SvmAddress>,
    ) -> Result<<Self::Scheme as Scheme>::Payload, Self::Error> {
        self.sign_selected_payment(selected).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use solana_keypair::Keypair;
    use solana_pubkey::pubkey;
    use url::Url;

    use crate::networks::svm::assets::UsdcSolanaDevnet;

    use super::*;

    #[tokio::test]
    async fn test_signing() {
        let keypair = Keypair::new();

        let svm_signer = ExactSvmSigner::builder()
            .signer(keypair)
            .asset(UsdcSolanaDevnet)
            .blockhash(Hash::default())
            .compute_unit_limit(5)
            .compute_unit_price(1)
            .build();

        let payment_selection = PaymentSelection {
            max_amount_required: 1000u64.into(),
            resource: Url::parse("https://example.com/payment").unwrap(),
            description: "Test payment".to_string(),
            mime_type: "application/json".to_string(),
            pay_to: SvmAddress(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR")),
            max_timeout_seconds: 60,
            asset: UsdcSolanaDevnet::ASSET.address,
            output_schema: None,
            extra: Some(json!({
                "feePayer": "Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR"
            })),
        };

        let payload = svm_signer
            .sign(&payment_selection)
            .await
            .expect("Signing should succeed");

        // Verify the payload contains a valid base64-encoded transaction
        assert!(!payload.transaction.is_empty());

        // Decode and verify it's valid base64
        let decoded = BASE64_STANDARD
            .decode(&payload.transaction)
            .expect("Should be valid base64");
        assert!(!decoded.is_empty());

        // Deserialize and verify it's a valid transaction
        let transaction: Transaction =
            bincode::serde::decode_from_slice(&decoded, bincode::config::legacy())
                .expect("Should be valid transaction")
                .0;

        // Verify the transaction has at least one signature
        assert!(!transaction.signatures.is_empty());

        // Verify the signer's signature is present
        let signer_pubkey = TransactionSigner::pubkey(&svm_signer.signer).await;
        assert!(transaction.message.account_keys.contains(&signer_pubkey));
    }

    #[tokio::test]
    async fn test_signing_without_fee_payer() {
        let keypair = Keypair::new();
        let svm_signer = ExactSvmSigner::builder()
            .signer(keypair)
            .asset(UsdcSolanaDevnet)
            .blockhash(Hash::default())
            .compute_unit_limit(5)
            .compute_unit_price(1)
            .build();

        let payment_selection = PaymentSelection {
            max_amount_required: 500u64.into(),
            resource: Url::parse("https://example.com/payment").unwrap(),
            description: "Test payment without fee payer".to_string(),
            mime_type: "application/json".to_string(),
            pay_to: SvmAddress(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR")),
            max_timeout_seconds: 60,
            asset: UsdcSolanaDevnet::ASSET.address,
            output_schema: None,
            extra: None, // No fee payer specified
        };

        let payload = svm_signer
            .sign(&payment_selection)
            .await
            .expect("Signing should succeed");

        // Verify the payload is valid
        assert!(!payload.transaction.is_empty());

        let decoded = BASE64_STANDARD
            .decode(&payload.transaction)
            .expect("Should be valid base64");

        let transaction: Transaction =
            bincode::serde::decode_from_slice(&decoded, bincode::config::legacy())
                .expect("Should be valid transaction")
                .0;

        // Verify the signer is the fee payer when not specified
        assert_eq!(
            transaction.message.account_keys[0],
            TransactionSigner::pubkey(&svm_signer.signer).await
        );
    }

    #[tokio::test]
    async fn test_signing_with_custom_config() {
        let keypair = Keypair::new();
        let svm_signer = ExactSvmSigner::builder()
            .signer(keypair)
            .asset(UsdcSolanaDevnet)
            .blockhash(Hash::default())
            .compute_unit_limit(5)
            .compute_unit_price(1)
            .build();

        let payment_selection = PaymentSelection {
            max_amount_required: 1000u64.into(),
            resource: Url::parse("https://example.com/payment").unwrap(),
            description: "Test payment".to_string(),
            mime_type: "application/json".to_string(),
            pay_to: SvmAddress(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR")),
            max_timeout_seconds: 60,
            asset: UsdcSolanaDevnet::ASSET.address,
            output_schema: None,
            extra: None,
        };

        let payload = svm_signer
            .sign_selected_payment(&payment_selection)
            .await
            .expect("Signing should succeed");

        // Verify the payload is valid
        assert!(!payload.transaction.is_empty());
    }
}
