use anyhow::{anyhow, Result};
use base64::prelude::*;
use bincode;
use solana_sdk::{
    message::{
        v0::LoadedAddresses, SanitizedMessage, SanitizedVersionedMessage, SimpleAddressLoader,
        VersionedMessage,
    },
    reserved_account_keys::ReservedAccountKeys,
    transaction::VersionedTransaction,
};

pub struct TransactionDetails {
    pub versioned_transaction: VersionedTransaction,
    pub sanitized_message: SanitizedMessage,
}

pub fn deserialize_transaction_base64_into_transaction_details(
    transaction: &str,
) -> Result<TransactionDetails> {
    let base64_decoded_tx = BASE64_STANDARD
        .decode(transaction)
        .map_err(|error| anyhow!("Invalid transaction: {error}"))?;
    let versioned_transaction = bincode::deserialize::<VersionedTransaction>(&base64_decoded_tx)
        .map_err(|error| anyhow!("Invalid transaction: {error}"))?;

    // Check the instructions
    let sanitized_message =
        versioned_message_to_sanitized_message(versioned_transaction.message.clone())?;

    Ok(TransactionDetails {
        versioned_transaction,
        sanitized_message,
    })
}

pub fn versioned_message_to_sanitized_message(
    versioned_message: VersionedMessage,
) -> Result<SanitizedMessage> {
    let sanitized_versioned_message = SanitizedVersionedMessage::try_new(versioned_message)
        .map_err(|error| anyhow!("Invalid transaction: {error}"))?;

    let sanitized_message = SanitizedMessage::try_new(
        sanitized_versioned_message,
        SimpleAddressLoader::Enabled(LoadedAddresses::default()), // We do not support any compression for now
        &ReservedAccountKeys::empty_key_set(),
    )
    .map_err(|error| anyhow!("Invalid transaction: {error}"))?;

    Ok(sanitized_message)
}
