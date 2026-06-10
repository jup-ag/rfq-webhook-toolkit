//! Jupnet token helpers.

use core::mem::MaybeUninit;

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};

use crate::error::OrderEngineError;

pub const TOKEN_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub const TOKEN_2022_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::from_str("Tokenis9xgQh7yMRbNBnV6uFq7LANbuZJwebxWBWixf");

pub const NATIVE_MINT_ID: Pubkey =
    pinocchio_pubkey::from_str("So11111111111111111111111111111111111111112");

pub const SYSTEM_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::from_str("11111111111111111111111111111111");

// --- Jupnet state layout offsets ---

/// Offset of the `mint` pubkey in a token account.
pub const ACCOUNT_MINT_OFFSET: usize = 0;
/// Offset of the `owner` pubkey in a token account.
pub const ACCOUNT_OWNER_OFFSET: usize = 32;
/// Total length of a base token account on Jupnet (no extensions).
pub const BASE_ACCOUNT_LEN: usize = 213;

/// Total length of a base mint on Jupnet.
pub const BASE_MINT_LEN: usize = 106;

/// Offset of the `decimals` byte inside a Jupnet mint.
const MINT_DECIMALS_OFFSET: usize = 68;

/// TLV `ExtensionType` value for `TransferFeeConfig` (Token-2022 fork only).
const EXTENSION_TYPE_TRANSFER_FEE_CONFIG: u16 = 1;
/// `AccountType` discriminator for mints carrying Token-2022 extensions.
const ACCOUNT_TYPE_MINT: u8 = 1;

// --- Instruction discriminators ---

const TRANSFER_DISCRIMINATOR: u8 = 3;
const TRANSFER_CHECKED_DISCRIMINATOR: u8 = 12;

#[inline(always)]
pub fn is_token_program(program_id: &Pubkey) -> bool {
    program_id == &TOKEN_PROGRAM_ID || program_id == &TOKEN_2022_PROGRAM_ID
}

#[inline(always)]
pub fn is_token_2022(program_id: &Pubkey) -> bool {
    program_id == &TOKEN_2022_PROGRAM_ID
}

pub fn check_token_account(
    token_account: &AccountInfo,
    expected_program: &Pubkey,
    expected_mint: &Pubkey,
    expected_authority: &Pubkey,
) -> ProgramResult {
    if unsafe { token_account.owner() } != expected_program {
        return Err(OrderEngineError::InvalidTokenAccountOwner.into());
    }
    let data = token_account.try_borrow_data()?;
    if data.len() < BASE_ACCOUNT_LEN {
        return Err(OrderEngineError::InvalidTokenAccountData.into());
    }
    let mint: &Pubkey = unsafe { &*(data[ACCOUNT_MINT_OFFSET..].as_ptr() as *const Pubkey) };
    let owner: &Pubkey = unsafe { &*(data[ACCOUNT_OWNER_OFFSET..].as_ptr() as *const Pubkey) };
    if mint != expected_mint {
        return Err(OrderEngineError::InvalidTokenAccountMint.into());
    }
    if owner != expected_authority {
        return Err(OrderEngineError::InvalidTokenAccountAuthority.into());
    }
    Ok(())
}

pub fn read_mint_decimals_and_reject_transfer_fee(
    mint_account: &AccountInfo,
    current_epoch: u64,
) -> Result<u8, ProgramError> {
    let data = mint_account.try_borrow_data()?;
    if data.len() < BASE_MINT_LEN {
        return Err(OrderEngineError::InvalidTokenAccountData.into());
    }
    let decimals = data[MINT_DECIMALS_OFFSET];

    if data.len() <= BASE_ACCOUNT_LEN {
        return Ok(decimals);
    }
    if data[BASE_ACCOUNT_LEN] != ACCOUNT_TYPE_MINT {
        return Ok(decimals);
    }

    let mut cursor = BASE_ACCOUNT_LEN + 1;
    while cursor + 4 <= data.len() {
        let ext_type = u16::from_le_bytes([data[cursor], data[cursor + 1]]);
        let ext_len = u16::from_le_bytes([data[cursor + 2], data[cursor + 3]]) as usize;
        cursor += 4;
        if ext_type == 0 {
            break;
        }
        if cursor + ext_len > data.len() {
            return Err(OrderEngineError::InvalidTokenAccountData.into());
        }
        if ext_type == EXTENSION_TYPE_TRANSFER_FEE_CONFIG {
            const TFC_OLDER_OFFSET: usize = 72;
            const TFC_NEWER_OFFSET: usize = 90;
            if ext_len < TFC_NEWER_OFFSET + 18 {
                return Err(OrderEngineError::InvalidTokenAccountData.into());
            }
            let ext = &data[cursor..cursor + ext_len];
            let read_fee = |base: usize| -> (u64, u16) {
                let epoch = u64::from_le_bytes(ext[base..base + 8].try_into().unwrap());
                let bps = u16::from_le_bytes(ext[base + 16..base + 18].try_into().unwrap());
                (epoch, bps)
            };
            let (newer_epoch, newer_bps) = read_fee(TFC_NEWER_OFFSET);
            let (_, older_bps) = read_fee(TFC_OLDER_OFFSET);
            let active_bps = if current_epoch >= newer_epoch {
                newer_bps
            } else {
                older_bps
            };
            if active_bps != 0 {
                return Err(OrderEngineError::Token2022MintExtensionNotSupported.into());
            }
        }
        cursor += ext_len;
    }
    Ok(decimals)
}

pub fn transfer(
    token_program: &AccountInfo,
    from: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    amount: u128,
    signers: &[Signer],
) -> ProgramResult {
    let account_metas = [
        AccountMeta::writable(from.key()),
        AccountMeta::writable(to.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];

    let mut data: [MaybeUninit<u8>; 33] = [MaybeUninit::uninit(); 33];
    let bytes: &mut [u8; 33] = unsafe { &mut *(data.as_mut_ptr() as *mut [u8; 33]) };
    bytes[0] = TRANSFER_DISCRIMINATOR;
    write_amount(&mut bytes[1..33], amount);

    let instruction = Instruction {
        program_id: token_program.key(),
        accounts: &account_metas,
        data: &bytes[..],
    };
    invoke_signed(&instruction, &[from, to, authority], signers)
}

#[allow(clippy::too_many_arguments)]
pub fn transfer_checked(
    token_program: &AccountInfo,
    from: &AccountInfo,
    mint: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    amount: u128,
    decimals: u8,
    signers: &[Signer],
) -> ProgramResult {
    let account_metas = [
        AccountMeta::writable(from.key()),
        AccountMeta::readonly(mint.key()),
        AccountMeta::writable(to.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];

    let mut data: [MaybeUninit<u8>; 34] = [MaybeUninit::uninit(); 34];
    let bytes: &mut [u8; 34] = unsafe { &mut *(data.as_mut_ptr() as *mut [u8; 34]) };
    bytes[0] = TRANSFER_CHECKED_DISCRIMINATOR;
    write_amount(&mut bytes[1..33], amount);
    bytes[33] = decimals;

    let instruction = Instruction {
        program_id: token_program.key(),
        accounts: &account_metas,
        data: &bytes[..],
    };
    invoke_signed(&instruction, &[from, mint, to, authority], signers)
}

#[inline(always)]
fn write_amount(dst: &mut [u8], amount: u128) {
    debug_assert_eq!(dst.len(), 32);
    let lo = amount.to_le_bytes();
    dst[..16].copy_from_slice(&lo);
    for b in &mut dst[16..32] {
        *b = 0;
    }
}
