use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::VersionedMessage,
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};

use crate::{
    error::{Result, SquadsSdkError},
    serialize::deserialize_inner_instructions,
    transaction::{
        compiled_instructions, decode_transaction_base64, extract_compute_budget_params,
        uses_address_lookup_tables,
    },
    EXECUTE_TX_SYNC_V2_DISCRIMINATOR, SQUADS_PROGRAM_ID,
};

/// The result of unwrapping a Squads-wrapped transaction.
#[derive(Debug, Clone)]
pub struct UnwrappedTransaction {
    /// The inner swap instructions that were wrapped.
    pub instructions: Vec<Instruction>,
    /// The settings PDA used in the Squads instruction.
    pub settings_pda: Pubkey,
    /// The member pubkeys that were signers.
    pub members: Vec<Pubkey>,
    /// Number of signers encoded in the Squads instruction.
    pub num_signers: u8,
    /// Compute unit limit from the outer transaction's compute budget.
    pub compute_unit_limit: u32,
    /// Compute unit price from the outer transaction's compute budget.
    pub compute_unit_price: u64,
}

/// Unwrap a Squads V2 wrapped [`VersionedTransaction`] to recover the inner
/// instructions.
///
/// The transaction must be self-contained — if it uses address lookup tables,
/// this returns [`SquadsSdkError::AddressLookupTablesNotSupported`].
pub fn unwrap_transaction(tx: &VersionedTransaction) -> Result<UnwrappedTransaction> {
    unwrap_message(&tx.message)
}

/// Unwrap a Squads V2 wrapped [`VersionedMessage`] to recover the inner
/// instructions. See [`unwrap_transaction`] for details.
pub fn unwrap_message(message: &VersionedMessage) -> Result<UnwrappedTransaction> {
    if uses_address_lookup_tables(message) {
        return Err(SquadsSdkError::AddressLookupTablesNotSupported);
    }

    let account_keys = message.static_account_keys();
    let instructions = compiled_instructions(message);

    // Find the Squads execute instruction
    let squads_compiled = instructions
        .iter()
        .find(|compiled| {
            let program_index = usize::from(compiled.program_id_index);
            program_index < account_keys.len()
                && account_keys[program_index] == SQUADS_PROGRAM_ID
                && compiled.data.len() >= 8
                && compiled.data[..8] == EXECUTE_TX_SYNC_V2_DISCRIMINATOR
        })
        .ok_or(SquadsSdkError::UnrecognizedDiscriminator)?;

    // Parse the Squads instruction data:
    //   [disc:8][accountIndex:1][numSigners:1][padding:1][len:4][serialized_instructions...]
    let data = &squads_compiled.data;
    if data.len() < 15 {
        return Err(SquadsSdkError::ParseError(
            "squads instruction data too short".into(),
        ));
    }

    let num_signers = data[9];
    let payload_len = u32::from_le_bytes([data[11], data[12], data[13], data[14]]) as usize;

    if data.len() < 15 + payload_len {
        return Err(SquadsSdkError::ParseError(
            "squads instruction payload truncated".into(),
        ));
    }

    let payload = &data[15..15 + payload_len];

    // The Squads instruction accounts list (from the compiled instruction):
    //   [settingsPda, SQUADS_PROGRAM_ID, ...members(reversed), ...remaining_accounts]
    let squads_account_indices = &squads_compiled.accounts;
    let num_signers_usize = usize::from(num_signers);

    let overhead = 2 + num_signers_usize;
    if squads_account_indices.len() < overhead {
        return Err(SquadsSdkError::ParseError(
            "not enough accounts in squads instruction".into(),
        ));
    }

    let resolve = |idx: u8| -> Result<Pubkey> {
        let i = usize::from(idx);
        account_keys.get(i).copied().ok_or_else(|| {
            SquadsSdkError::ParseError(format!(
                "account index {} out of range (total keys: {})",
                i,
                account_keys.len()
            ))
        })
    };

    let settings_pda = resolve(squads_account_indices[0])?;

    // Members are at indices 2..2+numSigners. They were prepended in forward order
    // with insert(0) at wrap time, so they appear reversed — flip back.
    let mut members = Vec::with_capacity(num_signers_usize);
    for i in 0..num_signers_usize {
        members.push(resolve(squads_account_indices[2 + i])?);
    }
    members.reverse();

    // remaining_accounts start after the overhead.
    let remaining_pubkeys: Vec<Pubkey> = squads_account_indices[overhead..]
        .iter()
        .map(|&idx| resolve(idx))
        .collect::<Result<Vec<_>>>()?;

    let inner_instructions = deserialize_inner_instructions(payload)?;

    let mut reconstructed = Vec::with_capacity(inner_instructions.len());
    for inner in inner_instructions {
        let program_idx = usize::from(inner.program_id_index);
        let program_id = *remaining_pubkeys.get(program_idx).ok_or_else(|| {
            SquadsSdkError::ParseError("inner instruction program index out of range".into())
        })?;

        let mut metas = Vec::with_capacity(inner.account_indices.len());
        for acct_idx in &inner.account_indices {
            let idx = usize::from(*acct_idx);
            let pubkey = *remaining_pubkeys.get(idx).ok_or_else(|| {
                SquadsSdkError::ParseError("inner instruction account index out of range".into())
            })?;
            // Squads wraps the vault PDA's signer flag and re-applies it at CPI time, so we
            // can't recover the original is_signer for the vault. Return readonly/non-signer
            // and let callers compare against the original instruction set if they need flags.
            metas.push(AccountMeta::new_readonly(pubkey, false));
        }

        reconstructed.push(Instruction {
            program_id,
            accounts: metas,
            data: inner.data,
        });
    }

    let (compute_unit_limit, compute_unit_price) = extract_compute_budget_params(message)?;

    Ok(UnwrappedTransaction {
        instructions: reconstructed,
        settings_pda,
        members,
        num_signers,
        compute_unit_limit,
        compute_unit_price,
    })
}

/// Convenience wrapper that decodes a base64 transaction, then unwraps it.
///
/// The transaction must be self-contained — see [`unwrap_transaction`].
pub fn unwrap_transaction_base64(tx_b64: &str) -> Result<UnwrappedTransaction> {
    let tx = decode_transaction_base64(tx_b64)?;
    unwrap_transaction(&tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::SquadsWrapConfig, wrap::build_squads_wrapped_transaction};
    use solana_sdk::{hash::Hash, pubkey};

    fn test_pubkeys() -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
        (
            pubkey!("8f1s1b4Y3CVP9vA8QFf8m6v3oc7Q5Q8m2Un9u9A34M2T"), // settings
            pubkey!("3q8J3wTVpd6fHiFcPfebP8Fd6hQfKd8QxJ5zhhWgE4n9"), // vault
            pubkey!("Dk9EdQJk3JxR5aVdS3tDqQnBk7LfMoT1n7Vm5R4n4fq4"), // member_a
            pubkey!("4C58H5fm5P5k2p4A6HRo25ykoPS2atdx2myTaYF9E1f3"), // member_b
            pubkey!("9xQeWvG816bUx9EPf2st4qGSe6P6xj6Yy7D6A6M6y8d"),  // swap_program
            pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),  // token_program
            pubkey!("GDrB6xfg2s7zNBi8W6vX4NQAz3gU8GdU4cf9jXhVJzjP"), // user_ata
        )
    }

    #[test]
    fn round_trip_wrap_then_unwrap() {
        let (settings, vault, member_a, member_b, swap_program, token_program, user_ata) =
            test_pubkeys();

        let original_instructions = vec![
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(user_ata, false),
                    AccountMeta::new(vault, true),
                ],
                data: vec![0xAA],
            },
            Instruction {
                program_id: swap_program,
                accounts: vec![
                    AccountMeta::new(vault, true),
                    AccountMeta::new(user_ata, false),
                    AccountMeta::new_readonly(token_program, false),
                ],
                data: vec![0xBB, 0xCC],
            },
        ];

        let config = SquadsWrapConfig {
            settings_pda: settings,
            vault_pda: vault,
            members: vec![member_a, member_b],
            threshold: 2,
        };

        let wrapped_tx = build_squads_wrapped_transaction(
            &original_instructions,
            &config,
            Hash::new_unique(),
            400_000,
            500_000,
        )
        .expect("wrap");

        let unwrapped = unwrap_transaction(&wrapped_tx).expect("unwrap");

        // Verify we got the right number of instructions back
        assert_eq!(unwrapped.instructions.len(), original_instructions.len());

        // Verify instruction data + pubkeys match
        for (original, recovered) in original_instructions.iter().zip(&unwrapped.instructions) {
            assert_eq!(original.program_id, recovered.program_id);
            assert_eq!(original.data, recovered.data);
            assert_eq!(original.accounts.len(), recovered.accounts.len());
            for (orig_meta, rec_meta) in original.accounts.iter().zip(&recovered.accounts) {
                assert_eq!(orig_meta.pubkey, rec_meta.pubkey);
            }
        }

        // Verify metadata
        assert_eq!(unwrapped.settings_pda, settings);
        assert_eq!(unwrapped.num_signers, 2);
        assert_eq!(unwrapped.members.len(), 2);
        assert!(unwrapped.members.contains(&member_a));
        assert!(unwrapped.members.contains(&member_b));
        assert_eq!(unwrapped.compute_unit_limit, 400_000);
        assert_eq!(unwrapped.compute_unit_price, 500_000);
    }

    #[test]
    fn unwrap_rejects_non_squads_transaction() {
        use solana_sdk::{
            compute_budget::ComputeBudgetInstruction,
            message::{self, VersionedMessage},
            signature::NullSigner,
        };

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

        let err = unwrap_transaction(&tx).expect_err("should reject non-Squads tx");
        assert!(
            matches!(err, SquadsSdkError::UnrecognizedDiscriminator),
            "expected UnrecognizedDiscriminator, got: {err}"
        );
    }

    #[test]
    fn unwrap_rejects_alt_messages() {
        use solana_sdk::{
            address_lookup_table::AddressLookupTableAccount,
            instruction::Instruction,
            message::{v0::Message, VersionedMessage},
            signature::NullSigner,
        };

        let (_settings, _vault, member_a, _member_b, swap_program, token_program, user_ata) =
            test_pubkeys();

        let alt = AddressLookupTableAccount {
            key: Pubkey::new_unique(),
            addresses: vec![user_ata, token_program],
        };

        // A no-op tx that uses an ALT — the unwrap path should refuse it before
        // even looking for a Squads instruction.
        let swap_ix = Instruction {
            program_id: swap_program,
            accounts: vec![AccountMeta::new_readonly(user_ata, false)],
            data: vec![1],
        };
        let message =
            Message::try_compile(&member_a, &[swap_ix], &[alt], Hash::new_unique()).unwrap();
        let signer = NullSigner::new(&member_a);
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&signer]).unwrap();

        let err = unwrap_transaction(&tx).expect_err("should reject ALT-using tx");
        assert!(
            matches!(err, SquadsSdkError::AddressLookupTablesNotSupported),
            "expected AddressLookupTablesNotSupported, got: {err}"
        );
    }

    #[test]
    fn round_trip_single_instruction() {
        let (settings, vault, member_a, member_b, swap_program, _token_program, user_ata) =
            test_pubkeys();

        let original = vec![Instruction {
            program_id: swap_program,
            accounts: vec![
                AccountMeta::new(vault, true),
                AccountMeta::new(user_ata, false),
            ],
            data: vec![1, 2, 3, 4, 5],
        }];

        let config = SquadsWrapConfig {
            settings_pda: settings,
            vault_pda: vault,
            members: vec![member_a, member_b],
            threshold: 2,
        };

        let wrapped = build_squads_wrapped_transaction(
            &original,
            &config,
            Hash::new_unique(),
            200_000,
            100_000,
        )
        .expect("wrap");

        let unwrapped = unwrap_transaction(&wrapped).expect("unwrap");

        assert_eq!(unwrapped.instructions.len(), 1);
        assert_eq!(unwrapped.instructions[0].program_id, swap_program);
        assert_eq!(unwrapped.instructions[0].data, vec![1, 2, 3, 4, 5]);
        assert_eq!(unwrapped.instructions[0].accounts.len(), 2);
        assert_eq!(unwrapped.instructions[0].accounts[0].pubkey, vault);
        assert_eq!(unwrapped.instructions[0].accounts[1].pubkey, user_ata);
    }
}
