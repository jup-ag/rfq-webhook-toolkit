use std::collections::HashMap;

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::error::{Result, SquadsSdkError};

#[derive(Clone, Copy, Debug)]
struct KeyMetaFlags {
    is_signer: bool,
    is_writable: bool,
    first_seen: usize,
}

pub fn compile_remaining_accounts(
    swap_instructions: &[Instruction],
    vault_pda: &Pubkey,
) -> Result<Vec<AccountMeta>> {
    let mut map: HashMap<Pubkey, KeyMetaFlags> = HashMap::new();
    let mut seen_counter: usize = 0;

    for ix in swap_instructions {
        map.entry(ix.program_id).or_insert_with(|| {
            let flags = KeyMetaFlags {
                is_signer: false,
                is_writable: false,
                first_seen: seen_counter,
            };
            seen_counter += 1;
            flags
        });

        for key in &ix.accounts {
            let entry = map.entry(key.pubkey).or_insert_with(|| {
                let flags = KeyMetaFlags {
                    is_signer: false,
                    is_writable: false,
                    first_seen: seen_counter,
                };
                seen_counter += 1;
                flags
            });
            entry.is_signer = entry.is_signer || key.is_signer;
            entry.is_writable = entry.is_writable || key.is_writable;
        }
    }

    let mut writable_signers = Vec::new();
    let mut readonly_signers = Vec::new();
    let mut writable_non_signers = Vec::new();
    let mut readonly_non_signers = Vec::new();

    let mut metas = map.into_iter().collect::<Vec<(Pubkey, KeyMetaFlags)>>();
    metas.sort_by_key(|(_, flags)| flags.first_seen);

    for (pubkey, mut flags) in metas {
        if pubkey == *vault_pda {
            flags.is_signer = false;
        }

        let account_meta = match (flags.is_signer, flags.is_writable) {
            (true, true) => AccountMeta::new(pubkey, true),
            (true, false) => AccountMeta::new_readonly(pubkey, true),
            (false, true) => AccountMeta::new(pubkey, false),
            (false, false) => AccountMeta::new_readonly(pubkey, false),
        };

        match (account_meta.is_signer, account_meta.is_writable) {
            (true, true) => writable_signers.push(account_meta),
            (true, false) => readonly_signers.push(account_meta),
            (false, true) => writable_non_signers.push(account_meta),
            (false, false) => readonly_non_signers.push(account_meta),
        }
    }

    if writable_signers.len()
        + readonly_signers.len()
        + writable_non_signers.len()
        + readonly_non_signers.len()
        > usize::from(u8::MAX)
    {
        return Err(SquadsSdkError::ParseError(
            "too many remaining accounts".into(),
        ));
    }

    let mut remaining_accounts = Vec::with_capacity(
        writable_signers.len()
            + readonly_signers.len()
            + writable_non_signers.len()
            + readonly_non_signers.len(),
    );
    remaining_accounts.extend(writable_signers);
    remaining_accounts.extend(readonly_signers);
    remaining_accounts.extend(writable_non_signers);
    remaining_accounts.extend(readonly_non_signers);

    Ok(remaining_accounts)
}
