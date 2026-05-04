use base64::{prelude::BASE64_STANDARD, Engine};
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    compute_budget,
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{self, VersionedMessage},
    pubkey::Pubkey,
    signature::NullSigner,
    transaction::VersionedTransaction,
};

use crate::{
    accounts::compile_remaining_accounts,
    config::{SquadsWrapConfig, WrapOptions},
    error::{Result, SquadsSdkError},
    serialize::serialize_swap_instructions,
    transaction::{
        compiled_instructions, decode_transaction_base64, decompile_instruction,
        extract_compute_budget_params,
    },
    EXECUTE_TX_SYNC_V2_DISCRIMINATOR, SQUADS_PROGRAM_ID,
};

pub fn build_squads_wrapped_transaction(
    swap_instructions: &[Instruction],
    config: &SquadsWrapConfig,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
    compute_unit_price: u64,
) -> Result<VersionedTransaction> {
    build_squads_wrapped_transaction_with_alts(
        swap_instructions,
        config,
        recent_blockhash,
        compute_unit_limit,
        compute_unit_price,
        &[],
    )
}

/// Build a Squads-wrapped transaction with Address Lookup Table support.
///
/// ALTs are used in the outer `VersionedMessage` only — they compress the
/// static account keys in the serialized message. The inner serialized
/// instructions reference accounts by index into `remaining_accounts`,
/// which is unaffected by ALTs.
pub fn build_squads_wrapped_transaction_with_alts(
    swap_instructions: &[Instruction],
    config: &SquadsWrapConfig,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
    compute_unit_price: u64,
    address_lookup_tables: &[AddressLookupTableAccount],
) -> Result<VersionedTransaction> {
    config.validate()?;

    let mut remaining_accounts = compile_remaining_accounts(swap_instructions, &config.vault_pda)?;

    // Squads CPI invoke_signed limit: 64 account_infos total.
    // Total = remaining_accounts + 2 (settingsPda, SQUADS_PROGRAM_ID) + members.
    let total_accounts = remaining_accounts.len() + 2 + config.members.len();
    if total_accounts > 64 {
        return Err(SquadsSdkError::CpiAccountLimitExceeded {
            inner: remaining_accounts.len(),
            overhead: 2 + config.members.len(),
            total: total_accounts,
        });
    }

    let serialized = serialize_swap_instructions(swap_instructions, &remaining_accounts)?;

    // Collect signers from the swap instructions that are neither members nor the
    // vault PDA (whose flag was already stripped). These accounts — typically the
    // maker — must be signers on the outer transaction so their `is_signer` flag
    // propagates through CPI when the Squads program executes the inner instructions.
    let other_signer_pubkeys: Vec<Pubkey> = remaining_accounts
        .iter()
        .filter(|meta| meta.is_signer && !config.members.contains(&meta.pubkey))
        .map(|meta| meta.pubkey)
        .collect();

    // Prepend members as readonly signers. Forward iteration with insert(0) reverses
    // them, matching ultra-api's unshift behavior. The Squads program strips numSigners
    // entries from the front at runtime.
    for member in config.members.iter() {
        remaining_accounts.insert(0, AccountMeta::new_readonly(*member, true));
    }

    if serialized.len() > usize::try_from(u32::MAX).unwrap_or(usize::MAX) {
        return Err(SquadsSdkError::ParseError(
            "serialized instruction payload too large".into(),
        ));
    }

    let mut data = Vec::with_capacity(8 + 1 + 1 + 1 + 4 + serialized.len());
    data.extend_from_slice(&EXECUTE_TX_SYNC_V2_DISCRIMINATOR);
    data.push(0u8); // accountIndex
    data.push(config.members.len() as u8); // numSigners — must match prepended member count
    data.push(0u8); // padding
    data.extend_from_slice(&(serialized.len() as u32).to_le_bytes());
    data.extend_from_slice(&serialized);

    let mut keys = Vec::with_capacity(2 + remaining_accounts.len());
    keys.push(AccountMeta::new(config.settings_pda, false));
    keys.push(AccountMeta::new_readonly(SQUADS_PROGRAM_ID, false));
    keys.extend(remaining_accounts);

    let execute_ix = Instruction {
        program_id: SQUADS_PROGRAM_ID,
        accounts: keys,
        data,
    };

    let message = message::v0::Message::try_compile(
        &config.members[0],
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
            ComputeBudgetInstruction::set_compute_unit_price(compute_unit_price),
            execute_ix,
        ],
        address_lookup_tables,
        recent_blockhash,
    )?;

    let other_signers: Vec<NullSigner> = other_signer_pubkeys.iter().map(NullSigner::new).collect();

    let member_signers: Vec<NullSigner> = config.members.iter().map(NullSigner::new).collect();

    let mut signer_refs: Vec<&NullSigner> = member_signers.iter().collect();
    signer_refs.extend(other_signers.iter());

    Ok(VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &signer_refs,
    )?)
}

pub fn wrap_transaction_base64(
    quote_tx_b64: &str,
    config: &SquadsWrapConfig,
    options: &WrapOptions,
) -> Result<(String, String)> {
    wrap_transaction_base64_with_alts(quote_tx_b64, config, options, &[])
}

/// Wrap a base64-encoded transaction with ALT support.
///
/// Decompiles the input transaction, strips compute budget instructions,
/// wraps the remaining instructions inside Squads `executeTransactionSyncV2`,
/// and compresses the outer message using the provided ALTs.
pub fn wrap_transaction_base64_with_alts(
    quote_tx_b64: &str,
    config: &SquadsWrapConfig,
    options: &WrapOptions,
    address_lookup_tables: &[AddressLookupTableAccount],
) -> Result<(String, String)> {
    let tx = decode_transaction_base64(quote_tx_b64)?;
    let (cu_limit, cu_price) = extract_compute_budget_params(&tx.message)?;
    let instructions = compiled_instructions(&tx.message);

    let mut swap_instructions = Vec::new();
    for compiled in instructions {
        let ix = decompile_instruction(&tx.message, compiled)?;
        if ix.program_id != compute_budget::id() {
            swap_instructions.push(ix);
        }
    }

    let squads_cu_limit = cu_limit
        .saturating_mul(options.cu_multiplier)
        .min(options.cu_cap);

    let wrapped = build_squads_wrapped_transaction_with_alts(
        &swap_instructions,
        config,
        *tx.message.recent_blockhash(),
        squads_cu_limit,
        cu_price,
        address_lookup_tables,
    )?;

    let wrapped_message_b64 = BASE64_STANDARD.encode(wrapped.message.serialize());
    let wrapped_tx_bytes = bincode::serialize(&wrapped)
        .map_err(|e| SquadsSdkError::InvalidTransaction(format!("failed to serialize tx: {e}")))?;

    if wrapped_tx_bytes.len() > options.tx_size_limit {
        return Err(SquadsSdkError::TransactionSizeExceeded {
            size: wrapped_tx_bytes.len(),
            limit: options.tx_size_limit,
        });
    }

    let wrapped_tx_b64 = BASE64_STANDARD.encode(&wrapped_tx_bytes);

    Ok((wrapped_tx_b64, wrapped_message_b64))
}

/// Convenience wrapper that uses default [`WrapOptions`].
///
/// Returns `(wrapped_tx_base64, wrapped_message_base64)`.
pub fn wrap_quote_transaction_base64(
    quote_tx_b64: &str,
    config: &SquadsWrapConfig,
) -> Result<(String, String)> {
    wrap_transaction_base64(quote_tx_b64, config, &WrapOptions::default())
}

/// Preflight check: can the given instructions be wrapped into a Squads
/// transaction without exceeding constraints?
///
/// Validates config, CPI account limit (<=64), and estimates whether the
/// serialized transaction would fit within `options.tx_size_limit`.
/// Returns `Ok(())` if wrappable, or a specific error explaining why not.
pub fn can_wrap(
    swap_instructions: &[Instruction],
    config: &SquadsWrapConfig,
    options: &WrapOptions,
) -> Result<()> {
    config.validate()?;

    let remaining_accounts = compile_remaining_accounts(swap_instructions, &config.vault_pda)?;

    let total_accounts = remaining_accounts.len() + 2 + config.members.len();
    if total_accounts > 64 {
        return Err(SquadsSdkError::CpiAccountLimitExceeded {
            inner: remaining_accounts.len(),
            overhead: 2 + config.members.len(),
            total: total_accounts,
        });
    }

    let serialized = serialize_swap_instructions(swap_instructions, &remaining_accounts)?;

    // Conservative size estimate (no ALT compression):
    // signatures: 64 bytes * num_members
    // message: header(3) + blockhash(32) + compact_array overhead(~3)
    //   + account_keys: 32 * total unique keys
    //   + instructions: compute budget (~20 bytes) + squads ix (15 + payload)
    let num_keys = total_accounts + 2; // +2 for compute budget program + squads program (may overlap)
    let estimated_size = 64 * config.members.len() // signatures
        + 3 + 32 + 3                               // message header + blockhash + compact arrays
        + 32 * num_keys                             // account keys (no ALT compression)
        + 20                                        // compute budget instructions
        + 15 + serialized.len(); // squads execute ix

    if estimated_size > options.tx_size_limit {
        return Err(SquadsSdkError::TransactionSizeExceeded {
            size: estimated_size,
            limit: options.tx_size_limit,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash, instruction::AccountMeta, instruction::Instruction, pubkey, pubkey::Pubkey,
    };

    use crate::accounts::compile_remaining_accounts;

    fn test_pubkeys() -> (
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
    ) {
        (
            pubkey!("8f1s1b4Y3CVP9vA8QFf8m6v3oc7Q5Q8m2Un9u9A34M2T"), // settings
            pubkey!("3q8J3wTVpd6fHiFcPfebP8Fd6hQfKd8QxJ5zhhWgE4n9"), // vault
            pubkey!("Dk9EdQJk3JxR5aVdS3tDqQnBk7LfMoT1n7Vm5R4n4fq4"), // member_a
            pubkey!("4C58H5fm5P5k2p4A6HRo25ykoPS2atdx2myTaYF9E1f3"), // member_b
            pubkey!("HviMBVH4L84zW7xKL8oSPcDbXrjLVyRkCiYUjcVCVACE"), // member_c
            pubkey!("9xQeWvG816bUx9EPf2st4qGSe6P6xj6Yy7D6A6M6y8d"),  // swap_program
            pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),  // token_program
            pubkey!("GDrB6xfg2s7zNBi8W6vX4NQAz3gU8GdU4cf9jXhVJzjP"), // user_ata
        )
    }

    fn simple_swap_ix(
        vault: Pubkey,
        user_ata: Pubkey,
        token_program: Pubkey,
        swap_program: Pubkey,
    ) -> Instruction {
        Instruction {
            program_id: swap_program,
            accounts: vec![
                AccountMeta::new(vault, true),
                AccountMeta::new(user_ata, false),
                AccountMeta::new_readonly(token_program, false),
            ],
            data: vec![1, 2, 3],
        }
    }

    #[test]
    fn wraps_swap_instructions_with_members_as_signers() {
        let (settings, vault, member_a, member_b, _, swap_program, token_program, user_ata) =
            test_pubkeys();

        let swap_ix = simple_swap_ix(vault, user_ata, token_program, swap_program);

        let tx = build_squads_wrapped_transaction(
            &[swap_ix],
            &SquadsWrapConfig {
                settings_pda: settings,
                vault_pda: vault,
                members: vec![member_a, member_b],
                threshold: 2,
            },
            Hash::new_unique(),
            400_000,
            500_000,
        )
        .expect("build squads tx");

        assert_eq!(tx.signatures.len(), 2);
        match tx.message {
            VersionedMessage::V0(message) => {
                assert_eq!(message.account_keys[0], member_a);
                assert!(message.account_keys.contains(&member_b));
                assert_eq!(message.instructions.len(), 3);
            }
            _ => panic!("expected v0 message"),
        }
    }

    #[test]
    fn two_of_three_multisig_uses_members_len_not_threshold() {
        let (settings, vault, member_a, member_b, member_c, swap_program, token_program, user_ata) =
            test_pubkeys();

        let swap_ix = simple_swap_ix(vault, user_ata, token_program, swap_program);

        let tx = build_squads_wrapped_transaction(
            &[swap_ix],
            &SquadsWrapConfig {
                settings_pda: settings,
                vault_pda: vault,
                members: vec![member_a, member_b, member_c],
                threshold: 2, // 2-of-3
            },
            Hash::new_unique(),
            400_000,
            500_000,
        )
        .expect("build squads tx");

        // All 3 members must be signers, not just threshold count
        assert_eq!(tx.signatures.len(), 3);

        match &tx.message {
            VersionedMessage::V0(message) => {
                // Fee payer should be member_a
                assert_eq!(message.account_keys[0], member_a);
                assert!(message.account_keys.contains(&member_b));
                assert!(message.account_keys.contains(&member_c));
                assert_eq!(message.instructions.len(), 3);

                // Verify numSigners in the execute instruction data = 3 (members.len), not 2 (threshold)
                let execute_ix = &message.instructions[2]; // CU limit, CU price, execute
                                                           // data layout: [disc:8][accountIndex:1][numSigners:1][padding:1][len:4][...]
                let num_signers = execute_ix.data[9]; // offset 9 = numSigners
                assert_eq!(
                    num_signers, 3,
                    "numSigners should be members.len() (3), not threshold (2)"
                );
            }
            _ => panic!("expected v0 message"),
        }
    }

    #[test]
    fn member_prepend_order_matches_ultra_api() {
        let (_settings, vault, member_a, member_b, member_c, swap_program, token_program, user_ata) =
            test_pubkeys();

        let swap_ix = simple_swap_ix(vault, user_ata, token_program, swap_program);

        let mut remaining = compile_remaining_accounts(&[swap_ix], &vault).unwrap();

        // Before members: remaining has swap accounts only
        let accounts_before_members = remaining.len();

        // Replicate the member prepend from build_squads_wrapped_transaction
        for member in [member_a, member_b, member_c].iter() {
            remaining.insert(0, AccountMeta::new_readonly(*member, true));
        }

        // Members should be reversed at the front (matching ultra-api unshift behavior)
        assert_eq!(
            remaining[0].pubkey, member_c,
            "first should be last member (reversed)"
        );
        assert_eq!(
            remaining[1].pubkey, member_b,
            "second should be middle member"
        );
        assert_eq!(
            remaining[2].pubkey, member_a,
            "third should be first member"
        );

        // Original accounts follow
        assert_eq!(remaining.len(), accounts_before_members + 3);
    }

    #[test]
    fn rejects_exceeding_64_account_cpi_limit() {
        let (settings, vault, member_a, member_b, _, _, _, _) = test_pubkeys();

        // Create an instruction with many unique accounts to exceed the 64-account limit
        let mut accounts = Vec::new();
        for i in 0..62u8 {
            let mut bytes = [0u8; 32];
            bytes[0] = i;
            bytes[1] = 1; // avoid system program collision
            accounts.push(AccountMeta::new_readonly(
                Pubkey::new_from_array(bytes),
                false,
            ));
        }
        let big_ix = Instruction {
            program_id: Pubkey::new_unique(),
            accounts,
            data: vec![0],
        };

        let result = build_squads_wrapped_transaction(
            &[big_ix],
            &SquadsWrapConfig {
                settings_pda: settings,
                vault_pda: vault,
                members: vec![member_a, member_b],
                threshold: 2,
            },
            Hash::new_unique(),
            400_000,
            500_000,
        );

        assert!(
            result.is_err(),
            "should reject when accounts exceed 64-account CPI limit"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("64-account CPI limit"),
            "error message should mention CPI limit, got: {}",
            err
        );
    }

    #[test]
    fn multi_instruction_wrapping_serializes_correctly() {
        let (settings, vault, member_a, member_b, _, swap_program, token_program, user_ata) =
            test_pubkeys();

        let ix1 = Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(user_ata, false),
                AccountMeta::new(vault, true),
            ],
            data: vec![0xAA],
        };
        let ix2 = Instruction {
            program_id: swap_program,
            accounts: vec![
                AccountMeta::new(vault, true),
                AccountMeta::new(user_ata, false),
                AccountMeta::new_readonly(token_program, false),
            ],
            data: vec![0xBB, 0xCC],
        };

        let tx = build_squads_wrapped_transaction(
            &[ix1, ix2],
            &SquadsWrapConfig {
                settings_pda: settings,
                vault_pda: vault,
                members: vec![member_a, member_b],
                threshold: 2,
            },
            Hash::new_unique(),
            400_000,
            500_000,
        )
        .expect("build squads tx with multiple instructions");

        match &tx.message {
            VersionedMessage::V0(message) => {
                assert_eq!(message.instructions.len(), 3); // CU limit + CU price + execute
                let execute_ix = &message.instructions[2];
                // Verify instruction count in serialized data
                // data: [disc:8][accountIndex:1][numSigners:1][padding:1][len:4][numIx:1][...]
                let ix_payload_offset = 8 + 1 + 1 + 1 + 4; // 15
                assert_eq!(
                    execute_ix.data[ix_payload_offset], 2,
                    "should contain 2 serialized inner instructions"
                );
            }
            _ => panic!("expected v0 message"),
        }
    }

    #[test]
    fn validate_rejects_empty_members() {
        let config = SquadsWrapConfig {
            settings_pda: Pubkey::new_unique(),
            vault_pda: Pubkey::new_unique(),
            members: vec![],
            threshold: 1,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_threshold_exceeding_members() {
        let config = SquadsWrapConfig {
            settings_pda: Pubkey::new_unique(),
            vault_pda: Pubkey::new_unique(),
            members: vec![Pubkey::new_unique()],
            threshold: 2,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_threshold() {
        let config = SquadsWrapConfig {
            settings_pda: Pubkey::new_unique(),
            vault_pda: Pubkey::new_unique(),
            members: vec![Pubkey::new_unique()],
            threshold: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn non_member_signer_included_in_outer_transaction() {
        let (settings, vault, member_a, member_b, _, swap_program, token_program, user_ata) =
            test_pubkeys();

        // The maker is a signer on the inner instruction (like the order-engine
        // Fill instruction) but is NOT a squads member.
        let maker = pubkey!("BfvJHsm36WTTbMXFqBUfKZJGDqnSrvGnRWAT4WHQFcVP");

        let swap_ix = Instruction {
            program_id: swap_program,
            accounts: vec![
                AccountMeta::new(vault, true), // taker (vault PDA, signer)
                AccountMeta::new(maker, true), // maker (non-member signer)
                AccountMeta::new(user_ata, false),
                AccountMeta::new_readonly(token_program, false),
            ],
            data: vec![1, 2, 3],
        };

        let tx = build_squads_wrapped_transaction(
            &[swap_ix],
            &SquadsWrapConfig {
                settings_pda: settings,
                vault_pda: vault,
                members: vec![member_a, member_b],
                threshold: 2,
            },
            Hash::new_unique(),
            400_000,
            500_000,
        )
        .expect("build should succeed when swap instructions have non-member signers");

        // 2 members + 1 maker = 3 required signatures
        assert_eq!(tx.signatures.len(), 3);
        match &tx.message {
            VersionedMessage::V0(message) => {
                let signer_keys =
                    &message.account_keys[..message.header.num_required_signatures as usize];
                assert!(
                    signer_keys.contains(&maker),
                    "maker must be in the signers section of the message"
                );
                assert!(signer_keys.contains(&member_a));
                assert!(signer_keys.contains(&member_b));
            }
            _ => panic!("expected v0 message"),
        }
    }
}
