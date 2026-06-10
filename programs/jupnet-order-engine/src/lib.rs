//! Jupnet RFQ order-engine program — Pinocchio-based implementation.
#![cfg_attr(target_os = "solana", no_std)]

pub mod error;
pub mod processor;
pub mod token_2022;

#[cfg(not(feature = "production"))]
pinocchio_pubkey::declare_id!("473HkSFbCjEDmjecgXsuGXt25VkJpUZj7gAxywVpU46c");

#[cfg(feature = "production")]
pinocchio_pubkey::declare_id!("61DFfeTKM7trxYcPQCM78bJ794ddZprZpAwAnLiwTpYH");

/// PDA seed for the temporary wSOL token account used during SOL unwraps.
pub const TEMPORARY_WSOL_TOKEN_ACCOUNT: &[u8] = b"temporary-wsol-token-account";

/// Tag for the `Fill` instruction.
pub const FILL_DISCRIMINATOR: u8 = 0;

#[cfg(all(target_os = "solana", feature = "bpf-entrypoint"))]
mod entrypoint {
    use pinocchio::{
        account_info::AccountInfo, default_allocator, program_entrypoint, pubkey::Pubkey,
        ProgramResult,
    };

    program_entrypoint!(process_instruction);
    default_allocator!();

    // Inlined no_std panic handler.
    #[panic_handler]
    fn handler(info: &core::panic::PanicInfo<'_>) -> ! {
        if let Some(location) = info.location() {
            unsafe {
                pinocchio::syscalls::sol_panic_(
                    location.file().as_ptr(),
                    location.file().len() as u64,
                    location.line() as u64,
                    location.column() as u64,
                )
            }
        } else {
            const PANICKED: &str = "** PANICKED **";
            pinocchio::log::sol_log(PANICKED);
            unsafe { pinocchio::syscalls::abort() }
        }
    }

    pub fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        super::process(program_id, accounts, instruction_data)
    }
}

use pinocchio::{account_info::AccountInfo, pubkey::Pubkey, ProgramResult};

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (&tag, rest) = instruction_data
        .split_first()
        .ok_or(pinocchio::program_error::ProgramError::InvalidInstructionData)?;

    match tag {
        FILL_DISCRIMINATOR => processor::fill::process(program_id, accounts, rest),
        _ => Err(pinocchio::program_error::ProgramError::InvalidInstructionData),
    }
}
