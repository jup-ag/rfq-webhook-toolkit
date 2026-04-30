use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[derive(Debug, Clone, Default)]
pub struct SyncTransactionMeta {
    pub program_id: Pubkey,
    pub smart_account_address: Pubkey,
    pub smart_account_setting_address: Pubkey,
    pub transaction_signers: Vec<Pubkey>,
    pub vault_index: u8,
    pub instructions: Vec<Instruction>,
}

fn build_sync_transaction_instructions(
    &self,
    sync_transaction_meta: &SyncTransactionMeta,
) -> SolanaAdapterResult<Instruction> {
    let SyncTransactionMeta {
        program_id,
        smart_account_address,
        smart_account_setting_address,
        transaction_signers,
        vault_index,
        instructions,
        ..
    } = sync_transaction_meta;

    // Compile the synchronous transaction details
    let sync_details = SynchronousTransactionDetails::compile_to_synchronous_message_and_accounts(
        smart_account_address,
        transaction_signers,
        instructions,
    );

    let args = VaultTransactionSyncArgs {
        vault_index: *vault_index,
        num_signers: transaction_signers.len() as u8,
        instructions: sync_details.instructions,
    };

    let mut accounts = vec![
        AccountMeta::new_readonly(*smart_account_setting_address, false),
        AccountMeta::new_readonly(*program_id, false),
    ];
    accounts.extend(sync_details.accounts);

    let instruction = Instruction {
        program_id: *program_id,
        accounts,
        data: get_sync_transaction_data(args)?,
    };

    Ok(instruction)
}
