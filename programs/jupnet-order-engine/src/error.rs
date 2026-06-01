use pinocchio::program_error::ProgramError;

/// Custom errors raised by the order-engine program.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderEngineError {
    InvalidCalculation = 0,
    MissingTemporaryWrappedSolTokenAccount = 1,
    Token2022MintExtensionNotSupported = 2,
    OrderExpired = 3,
    InvalidTokenAccountData = 4,
    InvalidTokenAccountMint = 5,
    InvalidTokenAccountAuthority = 6,
    InvalidTokenAccountOwner = 7,
    InvalidInputMint = 8,
    InvalidTokenProgram = 9,
    InvalidSystemProgram = 10,
    InvalidTemporaryWsolPda = 11,
    NotEnoughAccountKeys = 12,
}

impl From<OrderEngineError> for ProgramError {
    fn from(value: OrderEngineError) -> Self {
        ProgramError::Custom(value as u32)
    }
}
