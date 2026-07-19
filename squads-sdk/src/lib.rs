mod accounts;
pub mod config;
pub mod error;
pub mod pda;
pub mod serialize;
pub mod settings;
pub mod transaction;
pub mod unwrap;
pub mod wrap;

use sha2::{Digest, Sha256};
use solana_message::VersionedMessage;
use solana_pubkey::{pubkey, Pubkey};
use solana_transaction::versioned::VersionedTransaction;

pub const SQUADS_PROGRAM_ID: Pubkey = pubkey!("SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG");

pub const EXECUTE_TX_SYNC_V2_DISCRIMINATOR: [u8; 8] = [90, 81, 187, 81, 39, 70, 128, 78];

/// Compute an Anchor-style 8-byte discriminator from a Sighash string
/// (e.g. `b"global:execute_transaction_sync_v2"`).
pub fn get_discriminator_bytes(sighash: &[u8]) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(sighash);
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

// Re-exports for ergonomic use
pub use config::{SquadsWrapConfig, WrapOptions};
pub use error::SquadsSdkError;
pub use pda::{derive_settings_pda, derive_vault_pda};
pub use settings::{parse_squads_settings, MemberPermissions, SquadsMember, SquadsSettings};
pub use unwrap::{
    unwrap_message, unwrap_transaction, unwrap_transaction_base64, UnwrappedTransaction,
};
pub use wrap::{
    build_squads_wrapped_transaction, can_wrap, wrap_quote_transaction_base64,
    wrap_transaction_base64,
};

/// Check if a [`VersionedMessage`] contains a Squads
/// `executeTransactionSyncV2` instruction without performing a full unwrap.
pub fn is_squads_message(message: &VersionedMessage) -> bool {
    let account_keys = message.static_account_keys();
    let instructions = transaction::compiled_instructions(message);
    instructions.iter().any(|compiled| {
        let program_index = usize::from(compiled.program_id_index);
        program_index < account_keys.len()
            && account_keys[program_index] == SQUADS_PROGRAM_ID
            && compiled.data.len() >= 8
            && compiled.data[..8] == EXECUTE_TX_SYNC_V2_DISCRIMINATOR
    })
}

/// Check if a [`VersionedTransaction`] contains a Squads
/// `executeTransactionSyncV2` instruction without performing a full unwrap.
pub fn is_squads_transaction(tx: &VersionedTransaction) -> bool {
    is_squads_message(&tx.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_discriminator_matches_sha256() {
        let computed = get_discriminator_bytes(b"global:execute_transaction_sync_v2");
        assert_eq!(computed, EXECUTE_TX_SYNC_V2_DISCRIMINATOR);
    }

    #[test]
    fn is_squads_transaction_detects_wrapped_tx() {
        use solana_hash::Hash;
        use solana_instruction::{AccountMeta, Instruction};

        let settings = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let member = Pubkey::new_unique();
        let swap_program = Pubkey::new_unique();

        let swap_ix = Instruction {
            program_id: swap_program,
            accounts: vec![AccountMeta::new(vault, true)],
            data: vec![1, 2, 3],
        };

        let tx = build_squads_wrapped_transaction(
            &[swap_ix],
            &SquadsWrapConfig {
                settings_pda: settings,
                vault_pda: vault,
                members: vec![member],
                threshold: 1,
                fee_payer: None,
            },
            Hash::new_unique(),
            400_000,
            500_000,
        )
        .unwrap();

        assert!(is_squads_transaction(&tx));
    }

    #[test]
    fn is_squads_transaction_rejects_plain_tx() {
        use solana_compute_budget_interface::ComputeBudgetInstruction;
        use solana_hash::Hash;
        use solana_message::{self as message, VersionedMessage};
        use solana_signer::null_signer::NullSigner;

        let payer = Pubkey::new_unique();
        let message = message::v0::Message::try_compile(
            &payer,
            &[ComputeBudgetInstruction::set_compute_unit_limit(400_000)],
            &[],
            Hash::new_unique(),
        )
        .unwrap();

        let signer = NullSigner::new(&payer);
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&signer]).unwrap();

        assert!(!is_squads_transaction(&tx));
    }
}
