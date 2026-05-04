use solana_sdk::{
    compute_budget,
    instruction::{AccountMeta, CompiledInstruction, Instruction},
    message::VersionedMessage,
    transaction::VersionedTransaction,
};

use base64::{prelude::BASE64_STANDARD, Engine};

use crate::error::{Result, SquadsSdkError};

/// Decode a base64-encoded Solana transaction.
pub fn decode_transaction_base64(b64: &str) -> Result<VersionedTransaction> {
    let tx_bytes = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| SquadsSdkError::InvalidBase64(e.to_string()))?;
    bincode::deserialize(&tx_bytes).map_err(|e| SquadsSdkError::InvalidTransaction(e.to_string()))
}

/// Get the compiled instructions from a [`VersionedMessage`], regardless of variant.
pub fn compiled_instructions(message: &VersionedMessage) -> &[CompiledInstruction] {
    match message {
        VersionedMessage::V0(v0) => &v0.instructions,
        VersionedMessage::Legacy(legacy) => &legacy.instructions,
    }
}

fn is_signer(message: &VersionedMessage, account_index: usize) -> bool {
    let header = message.header();
    account_index < usize::from(header.num_required_signatures)
}

fn is_writable(message: &VersionedMessage, account_index: usize, account_keys_len: usize) -> bool {
    let header = message.header();
    let num_required = usize::from(header.num_required_signatures);
    let num_readonly_signed = usize::from(header.num_readonly_signed_accounts);
    let num_readonly_unsigned = usize::from(header.num_readonly_unsigned_accounts);

    if account_index < num_required {
        account_index < (num_required.saturating_sub(num_readonly_signed))
    } else {
        account_index < (account_keys_len.saturating_sub(num_readonly_unsigned))
    }
}

pub fn decompile_instruction(
    message: &VersionedMessage,
    compiled: &CompiledInstruction,
) -> Result<Instruction> {
    let account_keys = message.static_account_keys();
    let program_index = usize::from(compiled.program_id_index);
    if program_index >= account_keys.len() {
        return Err(SquadsSdkError::ParseError(
            "program id index out of range".into(),
        ));
    }

    let mut metas = Vec::with_capacity(compiled.accounts.len());
    for idx in &compiled.accounts {
        let index = usize::from(*idx);
        if index >= account_keys.len() {
            return Err(SquadsSdkError::ParseError(
                "account index out of range".into(),
            ));
        }
        let pubkey = account_keys[index];
        let writable = is_writable(message, index, account_keys.len());
        let signer = is_signer(message, index);
        let meta = if writable {
            AccountMeta::new(pubkey, signer)
        } else {
            AccountMeta::new_readonly(pubkey, signer)
        };
        metas.push(meta);
    }

    Ok(Instruction {
        program_id: account_keys[program_index],
        accounts: metas,
        data: compiled.data.clone(),
    })
}

pub fn extract_compute_budget_params(message: &VersionedMessage) -> Result<(u32, u64)> {
    let instructions = compiled_instructions(message);

    let account_keys = message.static_account_keys();
    let mut cu_limit: Option<u32> = None;
    let mut cu_price: Option<u64> = None;

    for compiled in instructions {
        // Skip instructions whose program_id resolves through ALTs (not in static keys).
        // Compute budget instructions always use static account keys.
        let program_index = usize::from(compiled.program_id_index);
        if program_index >= account_keys.len() {
            continue;
        }
        if account_keys[program_index] != compute_budget::id() {
            continue;
        }
        if compiled.data.is_empty() {
            continue;
        }

        match compiled.data[0] {
            // ComputeBudgetInstruction::SetComputeUnitLimit
            2 if compiled.data.len() >= 5 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&compiled.data[1..5]);
                cu_limit = Some(u32::from_le_bytes(bytes));
            }
            // ComputeBudgetInstruction::SetComputeUnitPrice
            3 if compiled.data.len() >= 9 => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&compiled.data[1..9]);
                cu_price = Some(u64::from_le_bytes(bytes));
            }
            _ => {}
        }
    }

    Ok((cu_limit.unwrap_or(400_000), cu_price.unwrap_or(500_000)))
}
