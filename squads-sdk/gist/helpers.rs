use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};
use smart_account_program::SyncTransactionArgsV2;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::error::SolanaAdapterResult;

use super::small_vec::SmallVec;

#[derive(BorshDeserialize, BorshSerialize, Debug, Clone)]
pub struct CustomCompiledInstruction {
    pub program_id_index: u8,
    pub accounts: SmallVec<u8, u8>,
    pub data: SmallVec<u16, u8>,
}

#[derive(Debug, Clone)]
pub struct SynchronousTransactionDetails {
    pub instructions: Vec<u8>,
    pub accounts: Vec<AccountMeta>,
}

impl SynchronousTransactionDetails {
    pub fn compile_to_synchronous_message_and_accounts(
        vault_pda: &Pubkey,
        members: &[Pubkey],
        instructions: &[Instruction],
    ) -> Self {
        let compiled_keys = CompiledKeys::compile_without_payer(instructions);

        // Add members first
        // https://github.com/Squads-Protocol/smart-account-program/blob/main/programs/squads_smart_account_program/src/instructions/transaction_execute_sync.rs#L33
        let mut unique_accounts: Vec<AccountMeta> = members
            .iter()
            .map(|member| AccountMeta::new_readonly(*member, true))
            .collect();

        // Add instruction accounts, ensuring no duplicate pubkeys
        for account in compiled_keys.get_account_metas() {
            // Check if an account with this pubkey already exists
            if !unique_accounts
                .iter()
                .any(|existing| existing.pubkey == account.pubkey)
            {
                unique_accounts.push(account.clone());
            } else {
                // Optionally, you might want to merge permissions (take most permissive)
                // Find the existing account and update its permissions if needed
                if let Some(existing) = unique_accounts
                    .iter_mut()
                    .find(|a| a.pubkey == account.pubkey)
                {
                    existing.is_signer |= account.is_signer;
                    existing.is_writable |= account.is_writable;
                }
            }
        }

        // Update vault permissions
        if let Some(vault_account) = unique_accounts
            .iter_mut()
            .find(|acc| acc.pubkey == *vault_pda)
        {
            vault_account.is_signer = false;
        }

        let mut compiled_instructions = Vec::new();

        // Compile instructions
        for ix in instructions {
            let compiled = compile_instruction(&unique_accounts, ix);
            compiled_instructions.push(compiled);
        }

        let mut instructions_data = Vec::new();
        serialize_instructions_for_sync_execution(&compiled_instructions, &mut instructions_data);

        SynchronousTransactionDetails {
            instructions: instructions_data,
            accounts: unique_accounts,
        }
    }

    pub fn compile_to_synchronous_message_and_accounts_v2_with_hooks(
        vault_pda: &Pubkey,
        members: &[Pubkey],
        pre_hook_accounts: &[AccountMeta],
        post_hook_accounts: &[AccountMeta],
        instructions: &[Instruction],
    ) -> Self {
        let compiled_keys = CompiledKeys::compile_without_payer(instructions);

        // Build instruction accounts first (staticAccountKeys equivalent)
        // This matches the TypeScript version which builds remainingAccounts with staticAccountKeys first
        let mut instruction_accounts: Vec<AccountMeta> = compiled_keys.get_account_metas();

        // Mark the vault as non-signer if it exists
        if let Some(vault_account) = instruction_accounts
            .iter_mut()
            .find(|acc| acc.pubkey == *vault_pda)
        {
            vault_account.is_signer = false;
        }

        // Compile instructions using instruction accounts only
        // The account indices in compiled instructions reference positions in this array
        let mut compiled_instructions = Vec::new();
        for ix in instructions {
            let compiled = compile_instruction(&instruction_accounts, ix);
            compiled_instructions.push(compiled);
        }

        let mut instructions_data = Vec::new();
        serialize_instructions_for_sync_execution(&compiled_instructions, &mut instructions_data);

        // Build final account array in correct order:
        // [members..., preHookAccounts..., postHookAccounts..., instructionAccounts...]
        let mut all_accounts: Vec<AccountMeta> = members
            .iter()
            .map(|member| AccountMeta::new_readonly(*member, true))
            .collect();

        // Insert preHookAccounts after members
        all_accounts.splice(
            members.len()..members.len(),
            pre_hook_accounts.iter().cloned(),
        );

        // Insert postHookAccounts after preHookAccounts
        all_accounts.splice(
            members.len() + pre_hook_accounts.len()..members.len() + pre_hook_accounts.len(),
            post_hook_accounts.iter().cloned(),
        );

        // Append instruction accounts at the end
        all_accounts.extend(instruction_accounts);

        SynchronousTransactionDetails {
            instructions: instructions_data,
            accounts: all_accounts,
        }
    }
}

#[derive(Debug)]
struct CompiledKeys {
    key_meta_pairs: Vec<(Pubkey, AccountMeta)>,
}

impl CompiledKeys {
    fn compile_without_payer(instructions: &[Instruction]) -> Self {
        let mut key_meta_pairs: Vec<(Pubkey, AccountMeta)> = Vec::new();

        for ix in instructions {
            Self::add_program_id(&mut key_meta_pairs, ix);
            Self::add_account_metas(&mut key_meta_pairs, ix);
        }

        Self::sort_key_pairs(&mut key_meta_pairs);
        Self { key_meta_pairs }
    }

    fn get_account_metas(&self) -> Vec<AccountMeta> {
        self.key_meta_pairs
            .iter()
            .map(|(_, meta)| meta.clone())
            .collect()
    }

    fn add_program_id(key_meta_pairs: &mut Vec<(Pubkey, AccountMeta)>, ix: &Instruction) {
        if !key_meta_pairs.iter().any(|(key, _)| key == &ix.program_id) {
            key_meta_pairs.push((
                ix.program_id,
                AccountMeta::new_readonly(ix.program_id, false),
            ));
        }
    }

    fn add_account_metas(key_meta_pairs: &mut Vec<(Pubkey, AccountMeta)>, ix: &Instruction) {
        for account_meta in &ix.accounts {
            if let Some(existing) = key_meta_pairs
                .iter_mut()
                .find(|(key, _)| key == &account_meta.pubkey)
            {
                existing.1.is_writable |= account_meta.is_writable;
                existing.1.is_signer |= account_meta.is_signer;
            } else {
                key_meta_pairs.push((account_meta.pubkey, account_meta.clone()));
            }
        }
    }

    fn sort_key_pairs(key_meta_pairs: &mut [(Pubkey, AccountMeta)]) {
        key_meta_pairs.sort_by(|a, b| {
            match (
                a.1.is_signer,
                b.1.is_signer,
                a.1.is_writable,
                b.1.is_writable,
            ) {
                (true, false, _, _) => std::cmp::Ordering::Less,
                (false, true, _, _) => std::cmp::Ordering::Greater,
                (_, _, true, false) => std::cmp::Ordering::Less,
                (_, _, false, true) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0),
            }
        });
    }
}

fn compile_instruction(
    unique_accounts: &[AccountMeta],
    ix: &Instruction,
) -> CustomCompiledInstruction {
    let account_indices: Vec<u8> = ix
        .accounts
        .iter()
        .map(|key| {
            unique_accounts
                .iter()
                .position(|acc| acc.pubkey == key.pubkey)
                .expect("Account not found in remaining accounts") as u8
        })
        .collect();

    let program_id_index = unique_accounts
        .iter()
        .position(|acc| acc.pubkey == ix.program_id)
        .expect("Program ID not found in remaining accounts") as u8;

    CustomCompiledInstruction {
        program_id_index,
        accounts: account_indices.into(),
        data: ix.data.clone().into(),
    }
}

// turns a vector of instructions into a smallvec of compiled instructions
fn serialize_instructions_for_sync_execution(
    ix: &Vec<CustomCompiledInstruction>,
    output: &mut Vec<u8>,
) {
    output.push(ix.len() as u8);
    for ix in ix {
        let _ = ix.serialize(output);
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VaultTransactionSyncArgs {
    pub vault_index: u8,
    pub num_signers: u8,
    pub instructions: Vec<u8>,
}

pub fn get_sync_transaction_data(args: VaultTransactionSyncArgs) -> SolanaAdapterResult<Vec<u8>> {
    let mut data = Vec::new();
    let discriminator = get_discriminator_bytes(b"global:execute_transaction_sync");
    data.extend(discriminator);
    args.serialize(&mut data)?;

    Ok(data)
}

pub fn get_sync_transaction_v2_data(args: SyncTransactionArgsV2) -> SolanaAdapterResult<Vec<u8>> {
    let mut data = Vec::new();
    let discriminator = get_discriminator_bytes(b"global:execute_transaction_sync_v2");
    data.extend(discriminator);
    args.serialize(&mut data)?;

    Ok(data)
}

pub fn get_discriminator_bytes(discriminator: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(discriminator);
    let hash = hasher.finalize();
    let discriminator = &hash[..8];
    discriminator.to_vec()
}
