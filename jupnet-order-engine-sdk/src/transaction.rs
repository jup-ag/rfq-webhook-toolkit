//! Tx decoding helpers — bincode-deserialize a base64-encoded
//! [`VersionedTransaction`] and wrap it in a [`SanitizedMessage`] so the
//! validation in [`crate::fill`] can walk decompiled instructions.
//!
//! Jupnet's `VersionedTransaction` is the same wire shape as upstream Solana's
//! at the message level — the difference is in the signatures (TypedSignature
//! instead of raw Ed25519) and a few address/program-id types renamed. This
//! module re-uses Jupnet's types so the decode round-trips work against
//! Jupnet-issued transactions.

use std::collections::HashSet;

use anyhow::{anyhow, Result};
use base64::prelude::*;
use jupnet_sdk::{
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
    let bytes = BASE64_STANDARD
        .decode(transaction)
        .map_err(|e| anyhow!("Invalid transaction: {e}"))?;
    let versioned_transaction = bincode::deserialize::<VersionedTransaction>(&bytes)
        .map_err(|e| anyhow!("Invalid transaction: {e}"))?;
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
        .map_err(|e| anyhow!("Invalid transaction: {e}"))?;
    SanitizedMessage::try_new(
        sanitized_versioned_message,
        SimpleAddressLoader::Enabled(LoadedAddresses::default()),
        // jupnet-sdk doesn't ship `agave-reserved-account-keys`; the upstream
        // SDK used it to seed a list of "reserved" pubkeys for sanitization
        // checks. An empty set is fine for our purposes — we don't rely on
        // built-in reservations, and our own validation walks every account.
        &HashSet::new(),
    )
    .map_err(|e| anyhow!("Invalid transaction: {e}"))
}
