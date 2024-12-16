use anchor_lang::error_code;

#[error_code]
pub enum OrderEngineError {
    InvalidCalculation,
    MissingTemporaryWrappedSolTokenAccount,
}
