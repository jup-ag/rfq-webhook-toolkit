use std::collections::HashMap;

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::error::{Result, SquadsSdkError};

pub fn serialize_swap_instructions(
    swap_instructions: &[Instruction],
    remaining_accounts: &[AccountMeta],
) -> Result<Vec<u8>> {
    if swap_instructions.len() > usize::from(u8::MAX) {
        return Err(SquadsSdkError::ParseError(
            "too many swap instructions".into(),
        ));
    }

    let mut indexes: HashMap<Pubkey, usize> = HashMap::new();
    for (idx, meta) in remaining_accounts.iter().enumerate() {
        indexes.insert(meta.pubkey, idx);
    }

    let estimated_size: usize = 1 + swap_instructions
        .iter()
        .map(|ix| 1 + 1 + ix.accounts.len() + 2 + ix.data.len())
        .sum::<usize>();
    let mut out = Vec::with_capacity(estimated_size);
    out.push(swap_instructions.len() as u8);

    for ix in swap_instructions {
        let program_idx = indexes.get(&ix.program_id).ok_or_else(|| {
            SquadsSdkError::ParseError("program id not found in remaining accounts".into())
        })?;
        if *program_idx > usize::from(u8::MAX) {
            return Err(SquadsSdkError::ParseError("program index overflow".into()));
        }

        if ix.accounts.len() > usize::from(u8::MAX) {
            return Err(SquadsSdkError::ParseError(
                "too many account indexes in instruction".into(),
            ));
        }
        if ix.data.len() > usize::from(u16::MAX) {
            return Err(SquadsSdkError::ParseError(
                "instruction data too large".into(),
            ));
        }

        out.push(*program_idx as u8);
        out.push(ix.accounts.len() as u8);

        for key in &ix.accounts {
            let key_idx = indexes.get(&key.pubkey).ok_or_else(|| {
                SquadsSdkError::ParseError("account not found in remaining accounts".into())
            })?;
            if *key_idx > usize::from(u8::MAX) {
                return Err(SquadsSdkError::ParseError("account index overflow".into()));
            }
            out.push(*key_idx as u8);
        }

        let len = ix.data.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&ix.data);
    }

    Ok(out)
}

/// A parsed inner instruction from the serialized binary format.
#[derive(Debug, Clone)]
pub struct InnerInstruction {
    pub program_id_index: u8,
    pub account_indices: Vec<u8>,
    pub data: Vec<u8>,
}

/// Deserialize the binary payload produced by [`serialize_swap_instructions`]
/// back into its component parts.
pub fn deserialize_inner_instructions(data: &[u8]) -> Result<Vec<InnerInstruction>> {
    if data.is_empty() {
        return Err(SquadsSdkError::ParseError(
            "empty serialized instructions".into(),
        ));
    }

    let mut pos = 0;
    let num_instructions = data[pos] as usize;
    pos += 1;

    let mut instructions = Vec::with_capacity(num_instructions);

    for _ in 0..num_instructions {
        if pos >= data.len() {
            return Err(SquadsSdkError::ParseError(
                "unexpected end of instruction data".into(),
            ));
        }
        let program_id_index = data[pos];
        pos += 1;

        if pos >= data.len() {
            return Err(SquadsSdkError::ParseError(
                "unexpected end of instruction data".into(),
            ));
        }
        let num_accounts = data[pos] as usize;
        pos += 1;

        if pos + num_accounts > data.len() {
            return Err(SquadsSdkError::ParseError(
                "unexpected end of instruction data".into(),
            ));
        }
        let account_indices = data[pos..pos + num_accounts].to_vec();
        pos += num_accounts;

        if pos + 2 > data.len() {
            return Err(SquadsSdkError::ParseError(
                "unexpected end of instruction data".into(),
            ));
        }
        let data_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;

        if pos + data_len > data.len() {
            return Err(SquadsSdkError::ParseError(
                "unexpected end of instruction data".into(),
            ));
        }
        let ix_data = data[pos..pos + data_len].to_vec();
        pos += data_len;

        instructions.push(InnerInstruction {
            program_id_index,
            account_indices,
            data: ix_data,
        });
    }

    Ok(instructions)
}
