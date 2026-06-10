//! `Fill` instruction handler — Jupnet edition.
//!
//! ## Wire format (after the 1-byte program tag)
//!
//! ```text
//! offset  size  field
//!   0      1    flags (bitmap of optional accounts)
//!   1     16    input_amount   (u128 little-endian)
//!  17     16    output_amount  (u128 little-endian)
//!  33      8    expire_at      (i64  little-endian)
//!  41      2    fee_bps        (u16  little-endian, reserved)
//! ```
//!
//! Flag bits:
//!
//! ```text
//! 0  taker_input_token_account present
//! 1  maker_input_token_account present
//! 2  taker_output_token_account present
//! 3  maker_output_token_account present
//! 4  temporary_wsol_token_account present
//! ```
//!
//! Account order (only present optional accounts appear):
//!
//! ```text
//! 0  taker                (signer, mut)
//! 1  maker                (signer, mut)
//! 2  input_mint
//! 3  input_token_program
//! 4  output_mint
//! 5  output_token_program
//! 6  system_program
//! 7+ optional accounts in the order shown by the flag bits above
//! ```
//!
//! Amounts are `u128` on the wire because Jupnet token programs encode token
//! amounts as 32-byte little-endian values. SOL paths (where the amount goes
//! to `system_program::Transfer`) cap at `u64::MAX` — anything larger is
//! rejected as an `ArithmeticOverflow`.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::{CreateAccount, Transfer as SystemTransfer};
use pinocchio_token::instructions::{CloseAccount, InitializeAccount3, SyncNative};

use crate::{
    error::OrderEngineError,
    token_2022::{
        check_token_account, is_token_2022, is_token_program,
        read_mint_decimals_and_reject_transfer_fee, transfer as token_transfer, transfer_checked,
        BASE_ACCOUNT_LEN, NATIVE_MINT_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID,
    },
    TEMPORARY_WSOL_TOKEN_ACCOUNT,
};

const FLAG_TAKER_INPUT_TA: u8 = 1 << 0;
const FLAG_MAKER_INPUT_TA: u8 = 1 << 1;
const FLAG_TAKER_OUTPUT_TA: u8 = 1 << 2;
const FLAG_MAKER_OUTPUT_TA: u8 = 1 << 3;
const FLAG_TEMP_WSOL_TA: u8 = 1 << 4;

const DATA_LEN: usize = 1 + 16 + 16 + 8 + 2;

struct FillArgs {
    flags: u8,
    input_amount: u128,
    output_amount: u128,
    expire_at: i64,
}

fn parse_args(data: &[u8]) -> Result<FillArgs, ProgramError> {
    if data.len() < DATA_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let flags = data[0];
    let input_amount = u128::from_le_bytes(data[1..17].try_into().unwrap());
    let output_amount = u128::from_le_bytes(data[17..33].try_into().unwrap());
    let expire_at = i64::from_le_bytes(data[33..41].try_into().unwrap());
    // data[41..43] = fee_bps, currently unused.
    Ok(FillArgs {
        flags,
        input_amount,
        output_amount,
        expire_at,
    })
}

pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let args = parse_args(data)?;

    if Clock::get()?.unix_timestamp > args.expire_at {
        return Err(OrderEngineError::OrderExpired.into());
    }

    let mut iter = accounts.iter();
    let taker = next_account(&mut iter)?;
    let maker = next_account(&mut iter)?;
    let input_mint = next_account(&mut iter)?;
    let input_token_program = next_account(&mut iter)?;
    let output_mint = next_account(&mut iter)?;
    let output_token_program = next_account(&mut iter)?;
    let system_program = next_account(&mut iter)?;

    let taker_input_ta = optional_account(&mut iter, args.flags & FLAG_TAKER_INPUT_TA != 0)?;
    let maker_input_ta = optional_account(&mut iter, args.flags & FLAG_MAKER_INPUT_TA != 0)?;
    let taker_output_ta = optional_account(&mut iter, args.flags & FLAG_TAKER_OUTPUT_TA != 0)?;
    let maker_output_ta = optional_account(&mut iter, args.flags & FLAG_MAKER_OUTPUT_TA != 0)?;
    let temp_wsol_ta = optional_account(&mut iter, args.flags & FLAG_TEMP_WSOL_TA != 0)?;

    if !taker.is_signer() || !taker.is_writable() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !maker.is_signer() || !maker.is_writable() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system_program.key() != &SYSTEM_PROGRAM_ID {
        return Err(OrderEngineError::InvalidSystemProgram.into());
    }
    if !is_token_program(input_token_program.key()) {
        return Err(OrderEngineError::InvalidTokenProgram.into());
    }
    if !is_token_program(output_token_program.key()) {
        return Err(OrderEngineError::InvalidTokenProgram.into());
    }

    if let Some(ta) = taker_input_ta {
        check_token_account(ta, input_token_program.key(), input_mint.key(), taker.key())?;
    }
    if let Some(ta) = maker_input_ta {
        check_token_account(ta, input_token_program.key(), input_mint.key(), maker.key())?;
    }
    if let Some(ta) = taker_output_ta {
        check_token_account(
            ta,
            output_token_program.key(),
            output_mint.key(),
            taker.key(),
        )?;
    }
    if let Some(ta) = maker_output_ta {
        check_token_account(
            ta,
            output_token_program.key(),
            output_mint.key(),
            maker.key(),
        )?;
    }

    // --- Input leg: taker -> maker ---
    match (taker_input_ta, maker_input_ta) {
        (None, None) => {
            require_native_mint(input_mint.key())?;
            SystemTransfer {
                from: taker,
                to: maker,
                lamports: u128_to_lamports(args.input_amount)?,
            }
            .invoke()?;
        }
        (None, Some(maker_in)) => {
            require_native_mint(input_mint.key())?;
            SystemTransfer {
                from: taker,
                to: maker_in,
                lamports: u128_to_lamports(args.input_amount)?,
            }
            .invoke()?;
            SyncNative {
                native_token: maker_in,
            }
            .invoke()?;
        }
        (Some(taker_in), None) => {
            require_native_mint(input_mint.key())?;
            unwrap_sol(
                maker,
                taker,
                taker_in,
                None,
                temp_wsol_ta,
                input_mint,
                input_token_program,
                system_program,
                args.input_amount,
            )?;
        }
        (Some(taker_in), Some(maker_in)) => {
            do_token_transfer(
                input_token_program,
                taker_in,
                maker_in,
                taker,
                input_mint,
                args.input_amount,
            )?;
        }
    }

    // --- Output leg: maker -> taker ---
    match (maker_output_ta, taker_output_ta) {
        (None, None) => {
            require_native_mint(output_mint.key())?;
            SystemTransfer {
                from: maker,
                to: taker,
                lamports: u128_to_lamports(args.output_amount)?,
            }
            .invoke()?;
        }
        (Some(maker_out), None) => {
            require_native_mint(output_mint.key())?;
            unwrap_sol(
                maker,
                maker,
                maker_out,
                Some(taker),
                temp_wsol_ta,
                output_mint,
                output_token_program,
                system_program,
                args.output_amount,
            )?;
        }
        (None, Some(taker_out)) => {
            require_native_mint(output_mint.key())?;
            SystemTransfer {
                from: maker,
                to: taker_out,
                lamports: u128_to_lamports(args.output_amount)?,
            }
            .invoke()?;
            SyncNative {
                native_token: taker_out,
            }
            .invoke()?;
        }
        (Some(maker_out), Some(taker_out)) => {
            do_token_transfer(
                output_token_program,
                maker_out,
                taker_out,
                maker,
                output_mint,
                args.output_amount,
            )?;
        }
    }

    Ok(())
}

fn next_account<'a, I>(iter: &mut I) -> Result<&'a AccountInfo, ProgramError>
where
    I: Iterator<Item = &'a AccountInfo>,
{
    iter.next()
        .ok_or(OrderEngineError::NotEnoughAccountKeys.into())
}

fn optional_account<'a, I>(
    iter: &mut I,
    present: bool,
) -> Result<Option<&'a AccountInfo>, ProgramError>
where
    I: Iterator<Item = &'a AccountInfo>,
{
    if present {
        Ok(Some(next_account(iter)?))
    } else {
        Ok(None)
    }
}

#[inline(always)]
fn require_native_mint(mint: &Pubkey) -> ProgramResult {
    if mint != &NATIVE_MINT_ID {
        return Err(OrderEngineError::InvalidInputMint.into());
    }
    Ok(())
}

/// Narrow a u128 amount to u64 lamports for system-program transfers; fail if
/// it doesn't fit.
#[inline(always)]
fn u128_to_lamports(amount: u128) -> Result<u64, ProgramError> {
    u64::try_from(amount).map_err(|_| ProgramError::ArithmeticOverflow)
}

fn do_token_transfer(
    token_program: &AccountInfo,
    from: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    mint: &AccountInfo,
    amount: u128,
) -> ProgramResult {
    if is_token_2022(token_program.key()) {
        let epoch = Clock::get()?.epoch;
        let decimals = read_mint_decimals_and_reject_transfer_fee(mint, epoch)?;
        transfer_checked(
            token_program,
            from,
            mint,
            to,
            authority,
            amount,
            decimals,
            &[],
        )
    } else {
        token_transfer(token_program, from, to, authority, amount, &[])
    }
}

#[allow(clippy::too_many_arguments)]
fn unwrap_sol(
    maker: &AccountInfo,
    sender: &AccountInfo,
    sender_token_account: &AccountInfo,
    receiver: Option<&AccountInfo>,
    temporary_wsol_token_account: Option<&AccountInfo>,
    wsol_mint: &AccountInfo,
    token_program: &AccountInfo,
    _system_program: &AccountInfo,
    amount: u128,
) -> ProgramResult {
    let temp = temporary_wsol_token_account
        .ok_or(OrderEngineError::MissingTemporaryWrappedSolTokenAccount)?;

    // The wSOL token account flow only works against the legacy SPL Token
    // program. Reject early if a caller wires Token-2022 here.
    if token_program.key() != &TOKEN_PROGRAM_ID {
        return Err(OrderEngineError::InvalidTokenProgram.into());
    }

    let maker_key = *maker.key();
    let (expected_temp, bump) = find_program_address(
        &[TEMPORARY_WSOL_TOKEN_ACCOUNT, maker_key.as_ref()],
        &crate::ID,
    );
    if temp.key() != &expected_temp {
        return Err(OrderEngineError::InvalidTemporaryWsolPda.into());
    }

    let bump_seed = [bump];
    let signer_seeds: [Seed; 3] = [
        Seed::from(TEMPORARY_WSOL_TOKEN_ACCOUNT),
        Seed::from(maker_key.as_ref()),
        Seed::from(&bump_seed[..]),
    ];
    let signers = [Signer::from(&signer_seeds[..])];

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(BASE_ACCOUNT_LEN);
    CreateAccount {
        from: maker,
        to: temp,
        lamports,
        space: BASE_ACCOUNT_LEN as u64,
        owner: &TOKEN_PROGRAM_ID,
    }
    .invoke_signed(&signers)?;

    InitializeAccount3 {
        account: temp,
        mint: wsol_mint,
        owner: &maker_key,
    }
    .invoke()?;

    token_transfer(
        token_program,
        sender_token_account,
        temp,
        sender,
        amount,
        &[],
    )?;

    CloseAccount {
        account: temp,
        destination: maker,
        authority: maker,
    }
    .invoke()?;

    if let Some(receiver) = receiver {
        SystemTransfer {
            from: maker,
            to: receiver,
            lamports: u128_to_lamports(amount)?,
        }
        .invoke()?;
    }

    Ok(())
}
