use std::{fmt::Debug, future::Future, str::FromStr};

use base64::{Engine, prelude::BASE64_STANDARD};
use bincode::{config::standard, error::EncodeError, serde::encode_to_vec};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_message::{Instruction, VersionedMessage, v0::Message};
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_transaction::versioned::VersionedTransaction;

use serde::Deserialize;

use crate::{
    concepts::SchemeSigner,
    networks::svm::{ExplicitSvmAsset, SvmAddress, TokenProgram},
    schemes::exact_svm::ExactSvmScheme,
};

/// Default compute unit limit for transactions.
const DEFAULT_COMPUTE_UNIT_LIMIT: u32 = 200_000;

/// Priority fee in micro-lamports (1 micro-lamport = 0.000001 lamports).
const PRIORITY_FEE_MICRO_LAMPORTS: u64 = 1;

/// The main signer implementation for the Exact SVM payment scheme.
///
/// This struct combines an asset type, a transaction signer, and an RPC client
/// to build and sign SPL token transfer transactions.
pub struct ExactSvmSigner<A, S>
where
    A: ExplicitSvmAsset,
    S: TransactionSigner,
{
    pub asset: A,
    pub signer: S,
    pub rpc_client: RpcClient,
}

impl<A, S> ExactSvmSigner<A, S>
where
    A: ExplicitSvmAsset,
    S: TransactionSigner,
{
    /// Creates a new `ExactSvmSigner` instance.
    pub fn new(asset: A, signer: S, rpc_client: RpcClient) -> Self {
        Self {
            asset,
            signer,
            rpc_client,
        }
    }
}

/// Trait for signing Solana transactions.
///
/// This trait abstracts over different signing implementations, allowing
/// for both local keypair signing and remote/hardware wallet signing.
pub trait TransactionSigner {
    type Error: std::error::Error;

    /// Returns the public key of the signer.
    fn pubkey(&self) -> Pubkey;

    /// Signs a versioned transaction.
    ///
    /// The `fee_payer` parameter is used to determine which signatures
    /// are required. The implementation should add the signer's signature
    /// to the transaction.
    fn sign_transaction(
        &self,
        fee_payer: &Pubkey,
        transaction: VersionedTransaction,
    ) -> impl Future<Output = Result<VersionedTransaction, Self::Error>>;
}

/// Blanket implementation for any type implementing `solana_signer::Signer`.
///
/// This implementation supports partial signing - it signs the transaction
/// with the signer's key and leaves a placeholder for the fee payer's signature
/// if the fee payer is different from the signer.
impl<S: solana_signer::Signer> TransactionSigner for S {
    type Error = solana_signer::SignerError;

    fn pubkey(&self) -> Pubkey {
        solana_signer::Signer::pubkey(self)
    }

    async fn sign_transaction(
        &self,
        _fee_payer: &Pubkey,
        transaction: VersionedTransaction,
    ) -> Result<VersionedTransaction, Self::Error> {
        let message = transaction.message;
        let signer_pubkey = solana_signer::Signer::pubkey(self);

        // Get the number of required signatures from the message header
        let num_required_signatures = message.header().num_required_signatures as usize;

        // Get the static account keys to determine signature positions
        let static_keys = message.static_account_keys();

        // Sign the message
        let message_data = message.serialize();
        let signature = solana_signer::Signer::try_sign_message(self, &message_data)?;

        // Create signatures array with placeholders
        let mut signatures = vec![solana_signature::Signature::default(); num_required_signatures];

        // Place the signature in the correct position based on the signer's pubkey
        for (i, key) in static_keys.iter().take(num_required_signatures).enumerate() {
            if key == &signer_pubkey {
                signatures[i] = signature;
            }
            // Fee payer position (usually index 0) gets a default signature placeholder
            // which will be filled in later by the fee payer
        }

        Ok(VersionedTransaction {
            signatures,
            message,
        })
    }
}

/// Deserialized extra fields from payment selection.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeserializedExtra {
    pub fee_payer: String,
}

/// Parameters for building a token transfer transaction.
#[derive(Debug, Clone)]
pub struct TransferParams {
    /// The public key of the fee payer.
    pub fee_payer: Pubkey,
    /// The public key of the source token account owner (signer).
    pub source_owner: Pubkey,
    /// The public key of the destination token account owner.
    pub destination_owner: Pubkey,
    /// The amount to transfer in the smallest unit.
    pub amount: u64,
    /// Whether the source ATA exists.
    pub source_ata_exists: bool,
}

/// Result of building a transaction, before signing.
#[derive(Debug)]
pub struct UnsignedTransferTransaction {
    /// The unsigned versioned transaction.
    pub transaction: VersionedTransaction,
    /// The fee payer public key.
    pub fee_payer: Pubkey,
}

// ============================================================================
// Transaction Building Functions
// ============================================================================

/// Builds an unsigned SPL token transfer transaction.
///
/// This function creates a complete unsigned transaction with:
/// - Compute budget instructions (unit limit and priority fee)
/// - Optional ATA creation instruction (if source doesn't exist)
/// - SPL token transfer_checked instruction
///
/// # Arguments
/// * `params` - Transfer parameters including addresses and amounts
/// * `recent_blockhash` - Recent blockhash for the transaction
///
/// # Returns
/// An `UnsignedTransferTransaction` ready for signing.
pub fn build_transfer_transaction<A: ExplicitSvmAsset>(
    params: &TransferParams,
    recent_blockhash: solana_hash::Hash,
) -> Result<UnsignedTransferTransaction, BuildTransactionError> {
    let instructions = build_transfer_instructions::<A>(params)?;

    let message = Message::try_compile(&params.fee_payer, &instructions, &[], recent_blockhash)?;
    let versioned_message = VersionedMessage::V0(message);
    let transaction = VersionedTransaction {
        signatures: Vec::new(),
        message: versioned_message,
    };

    Ok(UnsignedTransferTransaction {
        transaction,
        fee_payer: params.fee_payer,
    })
}

/// Builds the list of instructions for a token transfer.
///
/// This includes compute budget instructions, optional ATA creation,
/// and the transfer_checked instruction.
pub fn build_transfer_instructions<A: ExplicitSvmAsset>(
    params: &TransferParams,
) -> Result<Vec<Instruction>, BuildTransactionError> {
    let token_program = get_token_program::<A>();
    let mint = &A::ASSET.address.0;

    let source_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
        &params.source_owner,
        mint,
        &token_program,
    );

    let destination_ata =
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &params.destination_owner,
            mint,
            &token_program,
        );

    let mut instructions = Vec::with_capacity(4);

    // Add compute budget instructions
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
        DEFAULT_COMPUTE_UNIT_LIMIT,
    ));
    instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
        PRIORITY_FEE_MICRO_LAMPORTS,
    ));

    // Add ATA creation if needed
    if !params.source_ata_exists {
        instructions.push(
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &params.fee_payer,
                &params.source_owner,
                mint,
                &token_program,
            ),
        );
    }

    // Add transfer instruction
    let transfer_ix = get_transfer_checked_instruction::<A>(
        &source_ata,
        &destination_ata,
        &params.source_owner,
        params.amount,
    )?;
    instructions.push(transfer_ix);

    Ok(instructions)
}

/// Signs a transaction and encodes it as base64.
///
/// # Arguments
/// * `signer` - The transaction signer
/// * `unsigned_tx` - The unsigned transaction to sign
///
/// # Returns
/// A base64-encoded signed transaction string.
pub async fn sign_and_encode_transaction<S: TransactionSigner>(
    signer: &S,
    unsigned_tx: UnsignedTransferTransaction,
) -> Result<String, SignTransactionError<S>> {
    let signed_tx = signer
        .sign_transaction(&unsigned_tx.fee_payer, unsigned_tx.transaction)
        .await
        .map_err(SignTransactionError::SigningError)?;

    let serialized =
        encode_to_vec(&signed_tx, standard()).map_err(SignTransactionError::SerializeError)?;
    Ok(BASE64_STANDARD.encode(serialized))
}

/// Error type for transaction building.
#[derive(Debug, thiserror::Error)]
pub enum BuildTransactionError {
    #[error("Failed to compile transaction message: {0}")]
    MessageCompileError(#[from] solana_message::CompileError),

    #[error("Solana program error: {0}")]
    ProgramError(#[from] solana_program_error::ProgramError),
}

/// Error type for transaction signing.
#[derive(Debug, thiserror::Error)]
pub enum SignTransactionError<S: TransactionSigner> {
    #[error("Transaction signing error: {0}")]
    SigningError(S::Error),

    #[error("Failed to serialize transaction: {0}")]
    SerializeError(EncodeError),
}

// ============================================================================
// SchemeSigner Implementation
// ============================================================================

impl<A, S> SchemeSigner<SvmAddress> for ExactSvmSigner<A, S>
where
    A: ExplicitSvmAsset,
    S: TransactionSigner + Debug,
{
    type Error = ExactSvmSignerError<S>;
    type Scheme = ExactSvmScheme;

    async fn sign(
        &self,
        selected: &crate::concepts::PaymentSelection<SvmAddress>,
    ) -> Result<<Self::Scheme as crate::concepts::Scheme>::Payload, Self::Error> {
        // Extract fee payer from payment selection
        let fee_payer = Self::extract_fee_payer(selected)?;

        // Parse amount
        let amount: u64 = selected
            .max_amount_required
            .0
            .try_into()
            .map_err(|_| Self::Error::AmountOverflow)?;

        // Check if source ATA exists
        let source_ata_exists = self.check_source_ata_exists(&fee_payer).await?;

        // Build transfer parameters
        let params = TransferParams {
            fee_payer,
            source_owner: self.signer.pubkey(),
            destination_owner: selected.pay_to.0,
            amount,
            source_ata_exists,
        };

        // Get recent blockhash and build transaction
        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        let unsigned_tx = build_transfer_transaction::<A>(&params, recent_blockhash)?;

        // Sign and encode
        let transaction = sign_and_encode_transaction(&self.signer, unsigned_tx)
            .await
            .map_err(|e| match e {
                SignTransactionError::SigningError(err) => {
                    Self::Error::TransactionSigningError(err)
                }
                SignTransactionError::SerializeError(err) => Self::Error::SerializeError(err),
            })?;

        Ok(crate::schemes::exact_svm::ExplicitSvmPayload { transaction })
    }
}

// ============================================================================
// Helper Methods
// ============================================================================

impl<A, S> ExactSvmSigner<A, S>
where
    A: ExplicitSvmAsset,
    S: TransactionSigner,
{
    /// Extracts the fee payer public key from the payment selection's extra field.
    fn extract_fee_payer(
        selected: &crate::concepts::PaymentSelection<SvmAddress>,
    ) -> Result<Pubkey, ExactSvmSignerError<S>> {
        selected
            .extra
            .clone()
            .ok_or(ExactSvmSignerError::MissingFeePayer)
            .and_then(|extra| {
                serde_json::from_value::<DeserializedExtra>(extra)
                    .map_err(ExactSvmSignerError::DeserializeError)
            })
            .and_then(|extra| {
                Pubkey::from_str(&extra.fee_payer)
                    .map_err(ExactSvmSignerError::InvalidFeePayerPubkey)
            })
    }

    /// Checks if the source associated token account exists.
    async fn check_source_ata_exists(
        &self,
        _fee_payer: &Pubkey,
    ) -> Result<bool, ExactSvmSignerError<S>> {
        let token_program = get_token_program::<A>();
        let mint = &A::ASSET.address.0;
        let signer_pubkey = self.signer.pubkey();

        let source_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            &signer_pubkey,
            mint,
            &token_program,
        );

        let exists = self
            .rpc_client
            .get_account_with_commitment(&source_ata, self.rpc_client.commitment())
            .await?
            .value
            .is_some();

        Ok(exists)
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Configuration for ExactSvmSigner (reserved for future use).
pub struct ExactSvmSignerConfig {}

/// Comprehensive error type for ExactSvmSigner operations.
#[derive(Debug, thiserror::Error)]
pub enum ExactSvmSignerError<S: TransactionSigner> {
    #[error("Transaction signing error: {0}")]
    TransactionSigningError(S::Error),

    #[error("Missing fee payer in payment selection")]
    MissingFeePayer,

    #[error("Deserialization error: {0}")]
    DeserializeError(#[from] serde_json::Error),

    #[error("Invalid feePayer pubkey: {0}")]
    InvalidFeePayerPubkey(#[from] solana_pubkey::ParsePubkeyError),

    #[error("RPC error: {0}")]
    RpcError(#[from] solana_rpc_client_api::client_error::Error),

    #[error("Failed to compile transaction message: {0}")]
    MessageCompileError(#[from] solana_message::CompileError),

    #[error("Required amount exceeds u64 maximum value")]
    AmountOverflow,

    #[error("Failed to serialize transaction: {0}")]
    SerializeError(#[from] EncodeError),

    #[error("Solana program error: {0}")]
    ProgramError(#[from] solana_program_error::ProgramError),

    #[error("Failed to build transaction: {0}")]
    BuildError(#[from] BuildTransactionError),
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Returns the token program ID for the given asset.
pub fn get_token_program<A: ExplicitSvmAsset>() -> Pubkey {
    match A::TOKEN_PROGRAM {
        TokenProgram::SplToken => spl_token::ID,
        TokenProgram::SplToken2022 => spl_token_2022::ID,
    }
}

/// Creates a transfer_checked instruction for the given asset type.
pub fn get_transfer_checked_instruction<A: ExplicitSvmAsset>(
    src_ata: &Pubkey,
    dest_ata: &Pubkey,
    authority: &Pubkey,
    amount: u64,
) -> Result<Instruction, solana_program_error::ProgramError> {
    match A::TOKEN_PROGRAM {
        TokenProgram::SplToken => spl_token::instruction::transfer_checked(
            &spl_token::ID,
            src_ata,
            &A::ASSET.address.0,
            dest_ata,
            authority,
            &[],
            amount,
            A::ASSET.decimals,
        ),
        TokenProgram::SplToken2022 => spl_token_2022::instruction::transfer_checked(
            &spl_token_2022::ID,
            src_ata,
            &A::ASSET.address.0,
            dest_ata,
            authority,
            &[],
            amount,
            A::ASSET.decimals,
        ),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networks::svm::assets::UsdcSolanaDevnet;
    use solana_keypair::Keypair;

    /// Test building transfer instructions without RPC.
    #[test]
    fn test_build_transfer_instructions() {
        let source_owner = Pubkey::new_unique();
        let destination_owner = Pubkey::new_unique();
        let fee_payer = source_owner;

        let params = TransferParams {
            fee_payer,
            source_owner,
            destination_owner,
            amount: 1_000_000, // 1 USDC
            source_ata_exists: true,
        };

        let instructions = build_transfer_instructions::<UsdcSolanaDevnet>(&params)
            .expect("Failed to build instructions");

        // Should have: compute unit limit, compute unit price, transfer
        assert_eq!(instructions.len(), 3);
    }

    /// Test building transfer instructions with ATA creation.
    #[test]
    fn test_build_transfer_instructions_with_ata_creation() {
        let source_owner = Pubkey::new_unique();
        let destination_owner = Pubkey::new_unique();
        let fee_payer = source_owner;

        let params = TransferParams {
            fee_payer,
            source_owner,
            destination_owner,
            amount: 1_000_000,
            source_ata_exists: false,
        };

        let instructions = build_transfer_instructions::<UsdcSolanaDevnet>(&params)
            .expect("Failed to build instructions");

        // Should have: compute unit limit, compute unit price, create ATA, transfer
        assert_eq!(instructions.len(), 4);
    }

    /// Test building and signing a complete transaction.
    ///
    /// This test requires the following environment variables:
    /// - `SOLANA_RPC_URL`: The Solana RPC endpoint (e.g., devnet)
    /// - `SOLANA_PRIVATE_KEY`: Base58-encoded private key (optional, generates one if not set)
    #[tokio::test]
    #[ignore = "Requires SOLANA_RPC_URL environment variable"]
    async fn test_build_and_sign_transaction() {
        let rpc_url = std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());

        // Use provided private key or generate a new keypair
        let keypair = match std::env::var("SOLANA_PRIVATE_KEY") {
            Ok(key) => {
                let bytes = bs58::decode(&key)
                    .into_vec()
                    .expect("Invalid base58 private key");
                Keypair::try_from(bytes.as_slice()).expect("Invalid keypair bytes")
            }
            Err(_) => Keypair::new(),
        };

        let rpc_client = RpcClient::new(rpc_url);
        let destination_owner = Pubkey::new_unique();
        let signer_pubkey = solana_signer::Signer::pubkey(&keypair);

        let params = TransferParams {
            fee_payer: signer_pubkey,
            source_owner: signer_pubkey,
            destination_owner,
            amount: 1_000_000, // 1 USDC
            source_ata_exists: true,
        };

        // Get recent blockhash
        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .await
            .expect("Failed to get blockhash");

        // Build unsigned transaction
        let unsigned_tx = build_transfer_transaction::<UsdcSolanaDevnet>(&params, recent_blockhash)
            .expect("Failed to build transaction");

        // Sign and encode
        let encoded = sign_and_encode_transaction(&keypair, unsigned_tx)
            .await
            .expect("Failed to sign transaction");

        // Verify it's valid base64
        let decoded = BASE64_STANDARD
            .decode(&encoded)
            .expect("Failed to decode base64");
        assert!(!decoded.is_empty());

        println!("Successfully built and signed transaction");
        println!("Base64 encoded transaction length: {}", encoded.len());
    }

    /// Test that transaction can be simulated on devnet.
    ///
    /// This test requires:
    /// - `SOLANA_RPC_URL`: The Solana RPC endpoint
    /// - A funded wallet with SOL for fees and USDC tokens
    #[tokio::test]
    #[ignore = "Requires funded wallet and SOLANA_RPC_URL"]
    async fn test_simulate_transaction() {
        use bincode::serde::decode_from_slice;

        let rpc_url = std::env::var("SOLANA_RPC_URL").expect("SOLANA_RPC_URL must be set");
        let private_key =
            std::env::var("SOLANA_PRIVATE_KEY").expect("SOLANA_PRIVATE_KEY must be set");

        let bytes = bs58::decode(&private_key)
            .into_vec()
            .expect("Invalid base58 private key");
        let keypair = Keypair::try_from(bytes.as_slice()).expect("Invalid keypair bytes");
        let signer_pubkey = solana_signer::Signer::pubkey(&keypair);

        let rpc_client = RpcClient::new(rpc_url);
        let destination_owner = Pubkey::new_unique();

        let params = TransferParams {
            fee_payer: signer_pubkey,
            source_owner: signer_pubkey,
            destination_owner,
            amount: 1_000, // 0.001 USDC
            source_ata_exists: true,
        };

        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .await
            .expect("Failed to get blockhash");

        let unsigned_tx = build_transfer_transaction::<UsdcSolanaDevnet>(&params, recent_blockhash)
            .expect("Failed to build transaction");

        let encoded = sign_and_encode_transaction(&keypair, unsigned_tx)
            .await
            .expect("Failed to sign transaction");

        // Decode and simulate
        let decoded = BASE64_STANDARD.decode(&encoded).expect("Failed to decode");
        let (tx, _): (VersionedTransaction, _) =
            decode_from_slice(&decoded, standard()).expect("Failed to deserialize");

        let result = rpc_client.simulate_transaction(&tx).await;

        match result {
            Ok(response) => {
                println!("Simulation result: {:?}", response.value);
                if let Some(err) = response.value.err {
                    println!("Simulation error: {:?}", err);
                } else {
                    println!("Simulation successful!");
                }
            }
            Err(e) => {
                println!("RPC error during simulation: {:?}", e);
            }
        }
    }
}
