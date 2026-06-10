//! Maker-side validation library for the Pinocchio `order-engine` program
//! deployed on Jupnet devnet (`FbgvSw9p45GvhnGBngWFDbXYcLkV42PsdNv7XPu3L81n`).
//!
//! Mirrors the public surface of `order-engine-sdk`, but the internals target
//! the Jupnet wire format:
//!
//! - **Fill instruction discriminator** is a single byte (`0`) plus a flag
//!   bitmap selecting which optional ATAs are present, followed by `u128`
//!   input/output amounts (16 bytes each), `i64` expire_at, and `u16` fee_bps.
//! - **TransferChecked** is 34 bytes total (1 disc + 32 amount LE + 1 decimals).
//! - **Token amounts on token accounts** are `u128` (16 bytes) at offset 64,
//!   so balance reads must use a u128 reader.
//! - **VersionedTransaction** decodes through Jupnet's bincode-compatible type
//!   whose signature field is `Vec<TypedSignature>`, not `Vec<Signature>`.
//!
//! Public API to consumers:
//!   - [`Order`], [`ValidatedFill`], [`ValidatedSimilarFill`]
//!   - [`validate_fill_sanitized_message`]
//!   - [`validate_similar_fill_sanitized_message`]
//!   - [`transaction::deserialize_transaction_base64_into_transaction_details`]

pub mod fill;
pub mod transaction;

pub use fill::{
    peek_fill_expire_at, validate_fill_sanitized_message, validate_similar_fill_sanitized_message,
    Order, ValidatedFill, ValidatedSimilarFill,
};
pub use transaction::{
    deserialize_transaction_base64_into_transaction_details, TransactionDetails,
};
