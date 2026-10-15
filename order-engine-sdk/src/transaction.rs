use anyhow::{anyhow, Result};
use base64::prelude::*;
use bincode;
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    message::{
        v0::LoadedAddresses, SanitizedMessage, SanitizedVersionedMessage, SimpleAddressLoader,
        VersionedMessage,
    },
    transaction::VersionedTransaction,
};

pub struct TransactionDetails {
    pub versioned_transaction: VersionedTransaction,
    pub sanitized_message: SanitizedMessage,
}

pub fn deserialize_transaction_base64_into_transaction_details(
    transaction: &str,
) -> Result<TransactionDetails> {
    deserialize_transaction_base64_with_lookup_tables(transaction, &[])
}

/// Like [`deserialize_transaction_base64_into_transaction_details`], resolving address lookup
/// tables from `tables` (the caller fetched them; unknown tables are an error).
pub fn deserialize_transaction_base64_with_lookup_tables(
    transaction: &str,
    tables: &[AddressLookupTableAccount],
) -> Result<TransactionDetails> {
    let base64_decoded_tx = BASE64_STANDARD
        .decode(transaction)
        .map_err(|error| anyhow!("Invalid transaction: {error}"))?;
    let versioned_transaction = bincode::deserialize::<VersionedTransaction>(&base64_decoded_tx)
        .map_err(|error| anyhow!("Invalid transaction: {error}"))?;

    let loaded_addresses = resolve_loaded_addresses(&versioned_transaction.message, tables)?;
    let sanitized_message = versioned_message_to_sanitized_message_with_loaded_addresses(
        versioned_transaction.message.clone(),
        loaded_addresses,
    )?;

    Ok(TransactionDetails {
        versioned_transaction,
        sanitized_message,
    })
}

/// Sanitize a message that uses no address lookup tables.
pub fn versioned_message_to_sanitized_message(
    versioned_message: VersionedMessage,
) -> Result<SanitizedMessage> {
    versioned_message_to_sanitized_message_with_loaded_addresses(
        versioned_message,
        LoadedAddresses::default(),
    )
}

pub fn versioned_message_to_sanitized_message_with_loaded_addresses(
    versioned_message: VersionedMessage,
    loaded_addresses: LoadedAddresses,
) -> Result<SanitizedMessage> {
    let sanitized_versioned_message = SanitizedVersionedMessage::try_new(versioned_message)
        .map_err(|error| anyhow!("Invalid transaction: {error}"))?;

    let sanitized_message = SanitizedMessage::try_new(
        sanitized_versioned_message,
        SimpleAddressLoader::Enabled(loaded_addresses),
        // No reserved keys: the same as ReservedAccountKeys::empty_key_set().
        &Default::default(),
    )
    .map_err(|error| anyhow!("Invalid transaction: {error}"))?;

    Ok(sanitized_message)
}

/// Resolve the addresses a v0 message loads from lookup tables, in the order the runtime uses
/// (all writable lookups first, then all readonly). Legacy messages resolve to nothing.
pub fn resolve_loaded_addresses(
    message: &VersionedMessage,
    tables: &[AddressLookupTableAccount],
) -> Result<LoadedAddresses> {
    let VersionedMessage::V0(message) = message else {
        return Ok(LoadedAddresses::default());
    };
    let mut loaded = LoadedAddresses::default();
    for lookup in &message.address_table_lookups {
        let table = tables
            .iter()
            .find(|table| table.key == lookup.account_key)
            .ok_or_else(|| anyhow!("Unknown address lookup table {}", lookup.account_key))?;
        let resolve = |indexes: &[u8]| -> Result<Vec<_>> {
            indexes
                .iter()
                .map(|index| {
                    table
                        .addresses
                        .get(usize::from(*index))
                        .copied()
                        .ok_or_else(|| {
                            anyhow!("Lookup index {index} out of range for table {}", table.key)
                        })
                })
                .collect()
        };
        loaded.writable.extend(resolve(&lookup.writable_indexes)?);
        loaded.readonly.extend(resolve(&lookup.readonly_indexes)?);
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::v0,
        pubkey::Pubkey,
    };

    #[test]
    fn resolves_lookup_table_addresses_in_runtime_order() {
        let [payer, program, w1, w2, r1] = [(); 5].map(|_| Pubkey::new_unique());
        let table = AddressLookupTableAccount {
            key: Pubkey::new_unique(),
            addresses: vec![r1, w1, w2],
        };
        let ix = Instruction {
            program_id: program,
            accounts: vec![
                AccountMeta::new(w1, false),
                AccountMeta::new_readonly(r1, false),
                AccountMeta::new(w2, false),
            ],
            data: vec![1],
        };
        let message = VersionedMessage::V0(
            v0::Message::try_compile(&payer, &[ix], &[table.clone()], Hash::default()).unwrap(),
        );

        let loaded = resolve_loaded_addresses(&message, &[table]).unwrap();
        assert_eq!(loaded.writable, vec![w1, w2]);
        assert_eq!(loaded.readonly, vec![r1]);
        let sanitized =
            versioned_message_to_sanitized_message_with_loaded_addresses(message.clone(), loaded)
                .unwrap();
        let ixs = sanitized.decompile_instructions();
        let keys: Vec<Pubkey> = ixs[0].accounts.iter().map(|a| *a.pubkey).collect();
        assert_eq!(keys, vec![w1, r1, w2]);
        assert!(
            resolve_loaded_addresses(&message, &[]).is_err(),
            "unknown table"
        );
    }
}
