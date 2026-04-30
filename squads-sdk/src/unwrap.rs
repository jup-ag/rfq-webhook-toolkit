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

/// Unwrap a Squads V2 wrapped [`VersionedTransaction`] using the full resolved
/// account key list.
///
/// `account_keys` must be the complete ordered list of account pubkeys for the
/// transaction — static keys followed by ALT-resolved writable keys then
/// ALT-resolved readonly keys. This is the same order returned by RPC methods
/// like `getTransaction` in the `accountKeys` field.
///
/// For transactions **without** address lookup tables, you can use
/// [`unwrap_transaction`] instead, which derives the keys from the message.
pub fn unwrap_transaction_with_account_keys(
    tx: &VersionedTransaction,
    account_keys: &[Pubkey],
) -> Result<UnwrappedTransaction> {
    unwrap_message_with_account_keys(&tx.message, account_keys)
}

/// Unwrap a Squads V2 wrapped [`VersionedMessage`] using the full resolved
/// account key list.
///
/// Behaves the same as [`unwrap_transaction_with_account_keys`] but operates
/// directly on a [`VersionedMessage`] — useful when you have the message but
/// not the full transaction.
pub fn unwrap_message_with_account_keys(
    message: &VersionedMessage,
    account_keys: &[Pubkey],
) -> Result<UnwrappedTransaction> {
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
    // [disc:8][accountIndex:1][numSigners:1][padding:1][len:4][serialized_instructions...]
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
    // [settingsPda, SQUADS_PROGRAM_ID, ...members(numSigners reversed), ...remaining_accounts]
    let squads_account_indices = &squads_compiled.accounts;
    let num_signers_usize = usize::from(num_signers);

    // We need at least: settingsPda + SQUADS_PROGRAM_ID + numSigners members
    let overhead = 2 + num_signers_usize;
    if squads_account_indices.len() < overhead {
        return Err(SquadsSdkError::ParseError(
            "not enough accounts in squads instruction".into(),
        ));
    }

    let settings_pda_index = usize::from(squads_account_indices[0]);
    if settings_pda_index >= account_keys.len() {
        return Err(SquadsSdkError::ParseError(
            "settings PDA index out of range".into(),
        ));
    }
    let settings_pda = account_keys[settings_pda_index];

    // Members are at indices 2..2+numSigners (reversed from how they were prepended)
    let mut members = Vec::with_capacity(num_signers_usize);
    for i in 0..num_signers_usize {
        let idx = usize::from(squads_account_indices[2 + i]);
        if idx >= account_keys.len() {
            return Err(SquadsSdkError::ParseError(
                "member account index out of range".into(),
            ));
        }
        members.push(account_keys[idx]);
    }
    // The members were prepended in forward order with insert(0), so they appear reversed.
    // Reverse them back to original order.
    members.reverse();

    // remaining_accounts start after the overhead
    let remaining_start = overhead;
    let remaining_account_indices = &squads_account_indices[remaining_start..];

    // Resolve remaining account pubkeys from the full account key list.
    // We derive writable status from the remaining_accounts ordering convention:
    // writable_signers, readonly_signers, writable_non_signers, readonly_non_signers.
    // However, since the inner serialized instructions reference these by index, we
    // just need the pubkeys. Writable status is embedded in the remaining_accounts
    // ordering which the Squads program uses at runtime.
    let remaining_pubkeys: Vec<Pubkey> = remaining_account_indices
        .iter()
        .map(|&idx| {
            let index = usize::from(idx);
            if index >= account_keys.len() {
                return Err(SquadsSdkError::ParseError(format!(
                    "remaining account index {} out of range (total keys: {})",
                    index,
                    account_keys.len()
                )));
            }
            Ok(account_keys[index])
        })
        .collect::<Result<Vec<_>>>()?;

    // Deserialize the inner instructions
    let inner_instructions = deserialize_inner_instructions(payload)?;

    let num_inner = inner_instructions.len();
    let mut reconstructed = Vec::with_capacity(num_inner);
    for inner in inner_instructions {
        let program_idx = usize::from(inner.program_id_index);
        if program_idx >= remaining_pubkeys.len() {
            return Err(SquadsSdkError::ParseError(
                "inner instruction program index out of range".into(),
            ));
        }
        let program_id = remaining_pubkeys[program_idx];

        let mut metas = Vec::with_capacity(inner.account_indices.len());
        for acct_idx in &inner.account_indices {
            let idx = usize::from(*acct_idx);
            if idx >= remaining_pubkeys.len() {
                return Err(SquadsSdkError::ParseError(
                    "inner instruction account index out of range".into(),
                ));
            }
            // We can't perfectly recover signer/writable flags for ALT-resolved
            // accounts without the original instruction metadata. Use writable=false,
            // signer=false as safe defaults — callers who need exact flags should
            // compare against the original instruction set.
            metas.push(AccountMeta::new_readonly(remaining_pubkeys[idx], false));
        }

        reconstructed.push(Instruction {
            program_id,
            accounts: metas,
            data: inner.data, // move, no clone
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

/// Unwrap a Squads V2 wrapped [`VersionedTransaction`] to recover the inner instructions.
///
/// This works for Legacy messages and V0 messages **without** address lookup tables.
/// If the transaction uses ALTs, use [`unwrap_transaction_with_account_keys`] instead,
/// passing the full resolved account key list.
pub fn unwrap_transaction(tx: &VersionedTransaction) -> Result<UnwrappedTransaction> {
    unwrap_message_with_account_keys(&tx.message, tx.message.static_account_keys())
}

/// Unwrap a Squads V2 wrapped [`VersionedMessage`] to recover the inner instructions.
pub fn unwrap_message(message: &VersionedMessage) -> Result<UnwrappedTransaction> {
    unwrap_message_with_account_keys(message, message.static_account_keys())
}

/// Convenience wrapper that decodes a base64 transaction, then unwraps it.
///
/// Only works for transactions without address lookup tables. For ALT transactions,
/// decode manually and use [`unwrap_transaction_with_account_keys`].
pub fn unwrap_transaction_base64(tx_b64: &str) -> Result<UnwrappedTransaction> {
    let tx = decode_transaction_base64(tx_b64)?;
    unwrap_transaction(&tx)
}

/// Convenience wrapper that decodes a base64 transaction and unwraps it using the
/// provided full account key list (required for transactions with ALTs).
pub fn unwrap_transaction_base64_with_account_keys(
    tx_b64: &str,
    account_keys: &[Pubkey],
) -> Result<UnwrappedTransaction> {
    let tx = decode_transaction_base64(tx_b64)?;
    unwrap_transaction_with_account_keys(&tx, account_keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::SquadsWrapConfig, wrap::build_squads_wrapped_transaction};
    use base64::{prelude::BASE64_STANDARD, Engine};
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

        // Verify instruction data matches
        for (original, recovered) in original_instructions.iter().zip(&unwrapped.instructions) {
            assert_eq!(original.program_id, recovered.program_id);
            assert_eq!(original.data, recovered.data);
            assert_eq!(original.accounts.len(), recovered.accounts.len());

            // Verify account pubkeys match
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

        // Build a regular (non-Squads) transaction
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

        let result = unwrap_transaction(&tx);
        assert!(result.is_err());
        match result.unwrap_err() {
            SquadsSdkError::UnrecognizedDiscriminator => {}
            other => panic!("expected UnrecognizedDiscriminator, got: {other}"),
        }
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

    /// Unwrap a real mainnet Squads V2 transaction (uses ALTs).
    /// Tx: 5kdWZuVbbfY7...M855
    #[test]
    fn unwrap_real_mainnet_squads_transaction() {
        let tx_b64 = "Au2xQSHfJn+2wj2UXyVDKsh+AizyR1opKXXRmd5gIcHkEv+ods81D9/geHVaC3MuCUTXkRSeQa0oH6hj6tzr/gioy3L9eLVkKmNjxC76ExU0U/6F4boCGYbyP/3tfYHanOcGH1bWrzQotaoicmDnhGwO+CE1ycZKXxBNvxc6BsAEgAIBAwoJY8tYG0EIet8mKXQ1rq6w6WjL8PxN+YrGD0onnYB1YYCZS02zq8/l0MYlibCOwhZ7dps7kBBWnv17+5U0nSqpYfgkJBjyRmM2coxL6RzurZMdqXTFJHvNbssVIcoQcFdt+kDxUHqJnLt1p0uNFHPhdkDXx3X5S13oybOGbeiZWIJYwHQeHxkIA+r5TTIZgVN9hLL6NJ2nrAKehqGss5LXi48B75t4fM5trXbsdFwBzuw6JluEHxgKZRA10BzJ6of7fYamLwIJw6+YEL8kgnCbpH6XZgyM/EPi5Hy1vnfDHwMGRm/lIRcy/+ytunLDm+e8jOW7xfcSayxDmzpAAAAABHnVW/IxwG7udMVuzmgVB/2xst6j9I5RArHNola8E48Gfpx62lotJoPJMYMsMojaWpFLHPMIFmFC+wEBb/QcQaqtnWWvxywtL+/NG9a967NTCOH4pqgLh8BenA1Gd8m2AwcABQIJ9gMABwAJAz5rCwAAAAAACR8DCQEABgIKCwQMBQ0OEQ8QEggXGBMVFBocGxkdHx4WmQFaUbtRJ0aATgACAIoAAAAEDAIAAQwAAgAAAHC0twAAAAAADQUBAA4PDAkAk/F7ZPSErnb/DSoQAAECAwQOEQ8PEg0CExAFAgYHCA4UFRYXDw8YEBkJBgMKCwoLCw8XDRovANGYU5N8/tjpD4CWmAAAAAAAwrkMAAAAAAAyAAIAAAACAAAAeQAQJwABaAEQJwECDwMBAAABAAkDKb+VBypPvwTfcXWT5zi8zCEnKBXCRxqA2fn1VbDUhCUCJzgHExIAKAEXFD3ff3ub180agM8AY10XjaJ4m10aRhU7S2DRTJaD+zJtA9nb3AXa3sTY3eMUE2FSxyhNElhHUd7cMvgUJj7rkOQzjJyWkW4i2WDHAyIdIAIjHw==";

        // Full resolved account keys from RPC getTransaction response.
        // 10 static + 8 writable ALT + 14 readonly ALT = 32 total.
        let account_keys: Vec<Pubkey> = vec![
            pubkey!("devpNoNn6FCTp1S2gxUFaGa9vSagrWdkUoBVyVZ7ai4"), // 0 - signer 1
            pubkey!("9ezm3kzUXTLYXyfh5SAK4KP5dHKeQN7C1pc14rzNZA48"), // 1 - signer 2
            pubkey!("7bS1ESnzxiCYbygf4ForuGUjUGU7uPvqTYoVz1szg6UW"), // 2
            pubkey!("8QJmMPTmRJSLsGxjAXYDquEtWjVaKvBK8HVvV4Mcn1gB"), // 3
            pubkey!("9mpVcDHc8CrMxdR1Lmu5A5GE3nqdfgzorZiie2LJdomQ"), // 4
            pubkey!("APn9WAoAX6hqnkGsJyLdiYtAZxzY8Ywk1hULjVaQSauc"), // 5
            pubkey!("HviMBVH4L84zW7xKL8oSPcDbXrjLVyRkCiYUjcVCVACE"), // 6 - vault
            pubkey!("ComputeBudget111111111111111111111111111111"), // 7
            pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"), // 8
            pubkey!("SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG"), // 9 - Squads
            pubkey!("2oL6my4QDDCfpgJZX1bZV1NgbmuNptKdgcE8wJm6efgk"), // 10
            pubkey!("EpdaePzdqRkMtdZJquVPUWgyoJ5YEEpYALki6dv9VBrt"), // 11
            pubkey!("AeanNmmxpMEcSv3a3rKaRcrPjXDwpEiG37syPRyu3VJ2"), // 12
            pubkey!("AWKLq38dBA6JEP1bLBUrqcki3zePKNDiLX8SocMLSFMj"), // 13
            pubkey!("Fgcod1MMhVeuMYG9zrJcnucf54U32bcEj4t8v9eiG4HJ"), // 14
            pubkey!("2AusztjRJ2dcShL8xSUv2FrTqWbLe28UszzHptwrqh2e"), // 15
            pubkey!("F2KCaXcp7AoQtxTDvNEDCyMyWjSCAMWNzcyN9dsPfPs5"), // 16
            pubkey!("FnH8uVCgE8iGz4KQSEpQWdpLxzEENB2n2XHYUhUw13wc"), // 17
            pubkey!("11111111111111111111111111111111"),            // 18
            pubkey!("7iWnBRRhBCiNXXPhqiGzvvBkKrvFSWqqmxRyu9VyYBxE"), // 19
            pubkey!("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf"), // 20
            pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"), // 21
            pubkey!("jitodontfront11111111111JustUseJupiterU1tra"), // 22
            pubkey!("So11111111111111111111111111111111111111112"), // 23
            pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), // 24
            pubkey!("BNrK9LpEn65QA4TyBLVSMdngW3XHj3xLfFPwGdCBv8wV"), // 25
            pubkey!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE"), // 26
            pubkey!("HagefcrC63EBesXX9yFHWVscSwqu26LQtTB7RxyVSThj"), // 27
            pubkey!("JuprjznTrTSp2UFa3ZBUFgwdAmtZCq4MQCwysN55USD"), // 28
            pubkey!("Sysvar1nstructions1111111111111111111111111"), // 29
            pubkey!("2naph4yYn9nF8yddV2aTjwnGuMLbUcgVX8M6B4ckezPE"), // 30
            pubkey!("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA"), // 31
        ];

        let tx_bytes = BASE64_STANDARD.decode(tx_b64).expect("decode base64");
        let tx: VersionedTransaction = bincode::deserialize(&tx_bytes).expect("deserialize tx");

        let unwrapped = unwrap_transaction_with_account_keys(&tx, &account_keys)
            .expect("should unwrap real mainnet tx");

        // Known signers
        let signer_a = pubkey!("devpNoNn6FCTp1S2gxUFaGa9vSagrWdkUoBVyVZ7ai4");
        let signer_b = pubkey!("9ezm3kzUXTLYXyfh5SAK4KP5dHKeQN7C1pc14rzNZA48");

        // Vault (taker)
        let vault = pubkey!("HviMBVH4L84zW7xKL8oSPcDbXrjLVyRkCiYUjcVCVACE");

        // Verify members recovered correctly
        assert_eq!(unwrapped.num_signers, 2);
        assert_eq!(unwrapped.members.len(), 2);
        assert!(
            unwrapped.members[0] == signer_a,
            "should contain signer devpNoNn..."
        );
        assert!(
            unwrapped.members[1] == signer_b,
            "should contain signer 9ezm3k..."
        );

        // Verify we got inner swap instructions (not empty)
        assert!(
            !unwrapped.instructions.is_empty(),
            "should have inner instructions"
        );

        // The inner instructions should reference the vault as an account
        let vault_referenced = unwrapped
            .instructions
            .iter()
            .any(|ix| ix.accounts.iter().any(|meta| meta.pubkey == vault));
        assert!(
            vault_referenced,
            "inner instructions should reference the vault"
        );

        // Squads program ID should NOT appear as an inner instruction program_id
        let squads_as_inner = unwrapped
            .instructions
            .iter()
            .any(|ix| ix.program_id == crate::SQUADS_PROGRAM_ID);
        assert!(
            !squads_as_inner,
            "squads program should not be an inner instruction"
        );

        // Compute budget should have been extracted from outer tx
        assert!(
            unwrapped.compute_unit_limit > 0,
            "should have non-zero CU limit"
        );
        assert!(
            unwrapped.compute_unit_price > 0,
            "should have non-zero CU price"
        );

        // Settings PDA — index 3 in the Squads instruction accounts
        assert_eq!(
            unwrapped.settings_pda,
            pubkey!("8QJmMPTmRJSLsGxjAXYDquEtWjVaKvBK8HVvV4Mcn1gB")
        );
    }
}
