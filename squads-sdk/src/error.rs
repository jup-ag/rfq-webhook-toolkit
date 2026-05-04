use thiserror::Error;

#[derive(Debug, Error)]
pub enum SquadsSdkError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("inner instruction accounts ({inner}) plus Squads overhead ({overhead}) = {total} exceeds 64-account CPI limit")]
    CpiAccountLimitExceeded {
        inner: usize,
        overhead: usize,
        total: usize,
    },

    #[error("wrapped transaction size ({size} bytes) exceeds {limit} byte limit; route has too many accounts for Squads wrapping without ALT support")]
    TransactionSizeExceeded { size: usize, limit: usize },

    #[error("invalid base64: {0}")]
    InvalidBase64(String),

    #[error("invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("unrecognized squads instruction discriminator")]
    UnrecognizedDiscriminator,

    #[error("invalid settings account data: {0}")]
    InvalidSettingsData(String),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("solana error: {0}")]
    SolanaError(String),
}

impl From<solana_sdk::signer::SignerError> for SquadsSdkError {
    fn from(e: solana_sdk::signer::SignerError) -> Self {
        SquadsSdkError::SolanaError(e.to_string())
    }
}

impl From<solana_sdk::message::CompileError> for SquadsSdkError {
    fn from(e: solana_sdk::message::CompileError) -> Self {
        SquadsSdkError::SolanaError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SquadsSdkError>;
