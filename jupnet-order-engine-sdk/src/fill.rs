//! Validates that an inbound (taker-signed) Fill transaction matches the
//! order the maker quoted, on Jupnet semantics:
//!
//! Wire format of the Fill instruction (1-byte tag + flag bitmap + packed):
//!
//! ```text
//! offset  size  field
//!   0      1    discriminator = 0 (Fill)
//!   1      1    flags (bitmap of optional accounts: bits 0..4)
//!   2     16    input_amount  (u128 little-endian)
//!  18     16    output_amount (u128 little-endian)
//!  34      8    expire_at     (i64  little-endian)
//!  42      2    fee_bps       (u16  little-endian, reserved)
//! ```
//!
//! Account list:
//!
//! ```text
//!   0  taker        (signer, mut)
//!   1  maker        (signer, mut)
//!   2  input_mint
//!   3  input_token_program
//!   4  output_mint
//!   5  output_token_program
//!   6  system_program
//!   7+ optional ATAs in flag-bit order: taker_input, maker_input,
//!      taker_output, maker_output, temp_wsol_pda
//! ```

use anyhow::{anyhow, bail, ensure, Context, Result};
use jupnet_compute_budget_interface::ComputeBudgetInstruction;
use jupnet_sdk::{
    borsh1::try_from_slice_unchecked, message::SanitizedMessage, pubkey, pubkey::Pubkey,
    sysvar::instructions::BorrowedInstruction,
};
use jupnet_sdk_ids::{compute_budget, system_program};
use jupnet_system_interface::instruction::SystemInstruction;

// ---------------------------------------------------------------------------
// Pinocchio program + Jupnet constants
// ---------------------------------------------------------------------------

/// Jupnet order-engine program id (devnet).
pub const ORDER_ENGINE_ID: Pubkey =
    pubkey!("473HkSFbCjEDmjecgXsuGXt25VkJpUZj7gAxywVpU46c");

/// Jupnet's fork of legacy SPL Token.
pub const TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Jupnet's fork of Token-2022 (upstream `TokenzQdBN...` is not deployed).
pub const TOKEN_2022_PROGRAM_ID: Pubkey =
    pubkey!("Tokenis9xgQh7yMRbNBnV6uFq7LANbuZJwebxWBWixf");

/// SPL Associated Token Account program.
pub const ASSOCIATED_TOKEN_ACCOUNT_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// Wrapped SOL mint pubkey.
pub const NATIVE_MINT: Pubkey =
    pubkey!("So11111111111111111111111111111111111111112");

// --- Instruction discriminators / format constants ---

/// Single-byte tag of our `Fill` instruction.
const FILL_DISCRIMINATOR: u8 = 0;
/// Length of the Fill instruction data: 1 tag + 1 flags + 16 in + 16 out + 8
/// expire + 2 fee_bps.
const FILL_DATA_LEN: usize = 1 + 1 + 16 + 16 + 8 + 2;

const FLAG_TAKER_INPUT_TA: u8 = 1 << 0;
const FLAG_MAKER_INPUT_TA: u8 = 1 << 1;
const FLAG_TAKER_OUTPUT_TA: u8 = 1 << 2;
const FLAG_MAKER_OUTPUT_TA: u8 = 1 << 3;
const FLAG_TEMP_WSOL_TA: u8 = 1 << 4;

/// TransferChecked discriminator (Jupnet token programs).
const TRANSFER_CHECKED_DISCRIMINATOR: u8 = 12;
/// TransferChecked data length on Jupnet: 1 disc + 32 amount LE + 1 decimals.
const TRANSFER_CHECKED_DATA_LEN: usize = 1 + 32 + 1;

// ---------------------------------------------------------------------------
// Public order type
// ---------------------------------------------------------------------------

/// A quote handed out by the maker — the validator confirms the taker-signed
/// transaction matches this exactly.
pub struct Order {
    pub taker: Pubkey,
    pub maker: Pubkey,
    pub in_amount: u128,
    pub input_mint: Pubkey,
    pub out_amount: u128,
    pub output_mint: Pubkey,
    pub expire_at: i64,
    /// If set and not equal to the taker, an additional `system_transfer` /
    /// `transfer_checked` instruction is required to forward the output to
    /// this address.
    pub receiver: Option<Pubkey>,
    /// Expected decimals of the output mint — verified against the
    /// `TransferChecked` arg when the receiver leg is SPL.
    pub output_decimals: u8,
}

/// Parsed Fill arg payload (everything past the leading discriminator byte).
struct FillArgs {
    flags: u8,
    input_amount: u128,
    output_amount: u128,
    expire_at: i64,
}

fn parse_fill_args(data: &[u8]) -> Result<FillArgs> {
    ensure!(
        data.len() >= FILL_DATA_LEN,
        "Fill data too short: {} < {}",
        data.len(),
        FILL_DATA_LEN
    );
    ensure!(
        data[0] == FILL_DISCRIMINATOR,
        "Not a Fill discriminator (got {})",
        data[0]
    );
    let flags = data[1];
    let input_amount = u128::from_le_bytes(data[2..18].try_into().unwrap());
    let output_amount = u128::from_le_bytes(data[18..34].try_into().unwrap());
    let expire_at = i64::from_le_bytes(data[34..42].try_into().unwrap());
    // data[42..44] = fee_bps placeholder
    Ok(FillArgs {
        flags,
        input_amount,
        output_amount,
        expire_at,
    })
}

/// Locate the Fill instruction in `sanitized_message` and return its
/// `expire_at`. Used by makers who need to read the taker-built tx's expiry
/// before they can construct the `Order` to validate against.
pub fn peek_fill_expire_at(sanitized_message: &SanitizedMessage) -> Result<i64> {
    for BorrowedInstruction {
        program_id, data, ..
    } in sanitized_message.decompile_instructions()
    {
        if program_id == &ORDER_ENGINE_ID {
            return Ok(parse_fill_args(data)?.expire_at);
        }
    }
    bail!("No Fill instruction found in message")
}

/// Resolved account positions inside a Fill instruction's account list,
/// after accounting for the flag-bitmap-gated optionals.
struct FillAccountPositions<'a> {
    taker: &'a Pubkey,
    maker: &'a Pubkey,
    input_mint: &'a Pubkey,
    input_token_program: &'a Pubkey,
    output_mint: &'a Pubkey,
    output_token_program: &'a Pubkey,
    taker_input_ta: Option<&'a Pubkey>,
    taker_output_ta: Option<&'a Pubkey>,
}

fn resolve_fill_accounts<'a>(
    accounts: &'a [&'a Pubkey],
    flags: u8,
) -> Result<FillAccountPositions<'a>> {
    let [taker, maker, input_mint, input_token_program, output_mint, output_token_program, _system_program, rest @ ..] =
        accounts
    else {
        bail!("Fill instruction has too few accounts");
    };
    let mut cursor = 0usize;
    let mut take_if = |flag: u8| -> Result<Option<&'a Pubkey>> {
        if flags & flag != 0 {
            let pk = rest
                .get(cursor)
                .copied()
                .with_context(|| format!("Missing optional account for flag {flag:#b}"))?;
            cursor += 1;
            Ok(Some(pk))
        } else {
            Ok(None)
        }
    };
    let taker_input_ta = take_if(FLAG_TAKER_INPUT_TA)?;
    let _maker_input_ta = take_if(FLAG_MAKER_INPUT_TA)?;
    let taker_output_ta = take_if(FLAG_TAKER_OUTPUT_TA)?;
    let _maker_output_ta = take_if(FLAG_MAKER_OUTPUT_TA)?;
    let _temp_wsol_ta = take_if(FLAG_TEMP_WSOL_TA)?;
    Ok(FillAccountPositions {
        taker,
        maker,
        input_mint,
        input_token_program,
        output_mint,
        output_token_program,
        taker_input_ta,
        taker_output_ta,
    })
}

// ---------------------------------------------------------------------------
// validate_fill_sanitized_message
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ValidatedFill {
    pub compute_unit_limit: u32,
    pub compute_unit_price: u64,
}

struct FillExtracted {
    taker_output_ta: Option<Pubkey>,
    output_token_program: Pubkey,
}

/// Validate that `sanitized_message` is the exact Fill transaction the maker
/// quoted as `order`. Mirrors `order_engine_sdk::fill::validate_fill_sanitized_message`
/// in shape and intent.
pub fn validate_fill_sanitized_message(
    sanitized_message: &SanitizedMessage,
    order: Order,
) -> Result<ValidatedFill> {
    let fee_payer = sanitized_message.fee_payer();
    ensure!(
        fee_payer == &order.maker,
        "Fee payer was not the expected maker {} but was {fee_payer}",
        order.maker
    );

    let num_signers = sanitized_message
        .get_signature_details()
        .num_transaction_signatures();
    ensure!(
        num_signers == 2 || num_signers == 3,
        "Expected 2 or 3 signers, got {num_signers}"
    );

    // 2 signers: [maker, taker]
    // 3 signers: [maker, ata payer, taker]
    let taker_is_signer = sanitized_message
        .account_keys()
        .iter()
        .take(num_signers as usize)
        .skip(1)
        .any(|key| key == &order.taker);
    ensure!(taker_is_signer, "Taker is not among the signers");

    let expected_receiver = order.receiver.filter(|r| r != &order.taker);
    let mut fill_ix_found = false;
    let mut transfer_ix_found = false;
    let mut compute_unit_limit: Option<u32> = None;
    let mut compute_unit_price: Option<u64> = None;
    let mut fill_extracted: Option<FillExtracted> = None;

    for BorrowedInstruction {
        program_id,
        accounts,
        data,
    } in sanitized_message.decompile_instructions()
    {
        if program_id == &compute_budget::ID {
            let ix = try_from_slice_unchecked::<ComputeBudgetInstruction>(data)?;
            match ix {
                ComputeBudgetInstruction::SetComputeUnitLimit(limit) => {
                    ensure!(
                        compute_unit_limit.is_none(),
                        "Compute unit limit is already set"
                    );
                    compute_unit_limit = Some(limit);
                }
                ComputeBudgetInstruction::SetComputeUnitPrice(price) => {
                    ensure!(
                        compute_unit_price.is_none(),
                        "Compute unit price is already set"
                    );
                    compute_unit_price = Some(price);
                }
                _ => bail!("Unexpected compute budget instruction"),
            }
        } else if program_id == &ASSOCIATED_TOKEN_ACCOUNT_ID {
            // Only CreateIdempotent (data == [1]) is allowed and the funder
            // must not be the maker.
            ensure!(
                data == [1],
                "Incorrect associated token account program data"
            );
            ensure!(
                accounts.first().map(|am| am.pubkey) != Some(&order.maker),
                "Maker cannot fund the ATA creation"
            );
        } else if program_id == &ORDER_ENGINE_ID {
            ensure!(!fill_ix_found, "Duplicated fill instruction");
            fill_ix_found = true;

            let args = parse_fill_args(data)?;
            let pubkeys = accounts.iter().map(|a| a.pubkey).collect::<Vec<_>>();
            let positions = resolve_fill_accounts(&pubkeys, args.flags)?;

            ensure!(positions.taker == &order.taker, "Invalid taker");
            ensure!(positions.maker == &order.maker, "Invalid maker");
            ensure!(
                positions.input_mint == &order.input_mint,
                "Invalid input mint"
            );
            ensure!(
                positions.output_mint == &order.output_mint,
                "Invalid output mint"
            );
            ensure!(
                args.input_amount == order.in_amount,
                "Fill input_amount mismatch"
            );
            ensure!(
                args.output_amount == order.out_amount,
                "Fill output_amount mismatch"
            );
            ensure!(args.expire_at == order.expire_at, "Incorrect expiry");
            ensure!(
                positions.input_token_program == &TOKEN_PROGRAM_ID
                    || positions.input_token_program == &TOKEN_2022_PROGRAM_ID,
                "Unrecognised input token program"
            );
            ensure!(
                positions.output_token_program == &TOKEN_PROGRAM_ID
                    || positions.output_token_program == &TOKEN_2022_PROGRAM_ID,
                "Unrecognised output token program"
            );
            fill_extracted = Some(FillExtracted {
                taker_output_ta: positions.taker_output_ta.copied(),
                output_token_program: *positions.output_token_program,
            });
        } else if program_id == &system_program::ID {
            ensure!(
                !transfer_ix_found,
                "Duplicated receiver transfer instruction"
            );
            let receiver = expected_receiver.context("Unexpected transfer instruction")?;
            ensure!(
                order.output_mint == NATIVE_MINT,
                "Unexpected system_program transfer for non-native output"
            );
            let SystemInstruction::Transfer { lamports } =
                bincode::deserialize::<SystemInstruction>(data)
                    .map_err(|e| anyhow!("Invalid system instruction: {e}"))?
            else {
                bail!("Unexpected system program instruction");
            };
            let [from, to, ..] = accounts.as_slice() else {
                bail!("Not enough accounts in system transfer");
            };
            ensure!(
                *from.pubkey == order.taker,
                "Receiver transfer source must be taker"
            );
            ensure!(
                *to.pubkey == receiver,
                "Receiver transfer destination must be the receiver"
            );
            ensure!(
                u128::from(lamports) == order.out_amount,
                "Receiver transfer amount must equal out_amount"
            );
            transfer_ix_found = true;
        } else if program_id == &TOKEN_PROGRAM_ID || program_id == &TOKEN_2022_PROGRAM_ID {
            ensure!(
                !transfer_ix_found,
                "Duplicated receiver transfer instruction"
            );
            let receiver = expected_receiver.context("Unexpected transfer instruction")?;
            ensure!(
                order.output_mint != NATIVE_MINT,
                "Unexpected SPL transfer for native SOL output"
            );
            let fill = fill_extracted
                .as_ref()
                .context("Receiver transfer must follow the fill instruction")?;
            ensure!(
                program_id == &fill.output_token_program,
                "Receiver transfer token program does not match fill output token program"
            );

            // Jupnet TransferChecked: [12, amount(u128 in lower 16 of 32 LE), decimals]
            ensure!(
                data.len() == TRANSFER_CHECKED_DATA_LEN,
                "Receiver transfer data length {} != expected {}",
                data.len(),
                TRANSFER_CHECKED_DATA_LEN
            );
            ensure!(
                data[0] == TRANSFER_CHECKED_DISCRIMINATOR,
                "Only transfer_checked is allowed from the token program (got disc {})",
                data[0]
            );
            // First 16 bytes carry the meaningful amount; require the upper
            // 16 bytes to be zero (no value loss when folding to u128).
            ensure!(
                data[17..33].iter().all(|b| *b == 0),
                "Receiver transfer amount upper 128 bits must be zero"
            );
            let amount = u128::from_le_bytes(data[1..17].try_into().unwrap());
            let decimals = data[33];

            let [source, mint, destination, authority, ..] = accounts.as_slice() else {
                bail!("Not enough accounts in transfer_checked");
            };
            let expected_source = fill
                .taker_output_ta
                .context("Fill must include taker_output_ta when a receiver transfer is required")?;
            let expected_destination = derive_ata(&receiver, &order.output_mint, program_id);
            ensure!(
                *source.pubkey == expected_source,
                "Receiver transfer source must be the taker output token account from the fill ix"
            );
            ensure!(
                *mint.pubkey == order.output_mint,
                "Receiver transfer mint must equal output_mint"
            );
            ensure!(
                *destination.pubkey == expected_destination,
                "Receiver transfer destination must be the receiver's ATA"
            );
            ensure!(
                *authority.pubkey == order.taker,
                "Receiver transfer authority must be the taker"
            );
            ensure!(
                amount == order.out_amount,
                "Receiver transfer amount must equal out_amount"
            );
            ensure!(
                decimals == order.output_decimals,
                "Receiver transfer decimals must equal output_decimals"
            );
            transfer_ix_found = true;
        } else {
            bail!("Unexpected program id {program_id}");
        }
    }

    ensure!(fill_ix_found, "Missing fill instruction");
    ensure!(
        transfer_ix_found || expected_receiver.is_none(),
        "Missing transfer instruction for receiver"
    );

    Ok(ValidatedFill {
        compute_unit_limit: compute_unit_limit.context("Missing compute unit limit")?,
        compute_unit_price: compute_unit_price.context("Missing compute unit price")?,
    })
}

// ---------------------------------------------------------------------------
// validate_similar_fill_sanitized_message
// ---------------------------------------------------------------------------

#[derive(PartialEq, Debug)]
pub struct ValidatedSimilarFill {
    pub taker: Pubkey,
    pub input_amount: u128,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub taker_input_mint_token_account: Option<Pubkey>,
    pub expire_at: i64,
}

/// Like `validate_fill_sanitized_message`, but compares two messages and
/// allows the compute-budget instructions to differ (some wallets mutate
/// them). All other instructions must be byte-identical.
pub fn validate_similar_fill_sanitized_message(
    sanitized_message: SanitizedMessage,
    original_sanitized_message: SanitizedMessage,
) -> Result<ValidatedSimilarFill> {
    let header = sanitized_message.header();
    let original_header = original_sanitized_message.header();
    ensure!(
        original_header.num_required_signatures == header.num_required_signatures,
        "Number of required signatures did not match"
    );

    let mut account_keys_iter = sanitized_message.account_keys().iter();
    for original_signer in original_sanitized_message
        .account_keys()
        .iter()
        .take(usize::from(original_header.num_required_signatures))
    {
        let signer = account_keys_iter
            .next()
            .context("Not enough account keys to validate signer")?;
        ensure!(signer == original_signer, "Signer did not match");
    }

    let sanitized_instructions = sanitized_message.decompile_instructions();
    let original_instructions = original_sanitized_message.decompile_instructions();
    ensure!(
        sanitized_instructions.len() == original_instructions.len(),
        "Number of instructions must match the original"
    );

    let mut validated: Option<ValidatedSimilarFill> = None;

    for (
        index,
        (
            BorrowedInstruction {
                program_id: opi,
                accounts: oa,
                data: od,
            },
            BorrowedInstruction {
                program_id,
                accounts,
                data,
            },
        ),
    ) in original_instructions
        .into_iter()
        .zip(sanitized_instructions)
        .enumerate()
    {
        ensure!(
            program_id == opi,
            "Instruction program id mismatch at {index}, {opi}"
        );
        ensure!(
            accounts.len() == oa.len(),
            "Instruction accounts length mismatch at {index}, {opi}"
        );
        ensure!(
            accounts.iter().zip(&oa).all(|(a, b)| {
                a.pubkey == b.pubkey && a.is_signer == b.is_signer && a.is_writable == b.is_writable
            }),
            "Instruction accounts did not match the original at {index}, {opi}"
        );
        if opi == &compute_budget::ID {
            // Allow compute-budget instructions to mutate (price + limit can
            // change). Re-decode to ensure they're a known variant.
            let _ = try_from_slice_unchecked::<ComputeBudgetInstruction>(data)?;
            continue;
        }
        ensure!(
            data == od,
            "Instruction did not match the original at {index}, {opi}"
        );
        if program_id == &ORDER_ENGINE_ID {
            ensure!(validated.is_none(), "Duplicated fill instruction");
            let args = parse_fill_args(data)?;
            let pubkeys = accounts.iter().map(|a| a.pubkey).collect::<Vec<_>>();
            let positions = resolve_fill_accounts(&pubkeys, args.flags)?;
            validated = Some(ValidatedSimilarFill {
                taker: *positions.taker,
                input_amount: args.input_amount,
                input_mint: *positions.input_mint,
                output_mint: *positions.output_mint,
                taker_input_mint_token_account: positions.taker_input_ta.copied(),
                expire_at: args.expire_at,
            });
        }
    }

    validated.context("Missing validated fill instruction")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Standard ATA derivation: `find_program_address([wallet, token_program, mint], ATA_PROGRAM)`.
fn derive_ata(wallet: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_ACCOUNT_ID,
    )
    .0
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use jupnet_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::{
            v0::{self, LoadedAddresses},
            SanitizedVersionedMessage, SimpleAddressLoader, VersionedMessage,
        },
    };
    use jupnet_system_interface::instruction::transfer as system_transfer;

    use std::sync::atomic::{AtomicU64, Ordering};
    static HASH_COUNTER: AtomicU64 = AtomicU64::new(1);
    /// Jupnet's `Hash` lacks `new_unique()`; counter-seeded equivalent.
    fn unique_hash() -> Hash {
        let n = HASH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&n.to_le_bytes());
        Hash::new_from_array(bytes)
    }

    fn make_sanitized_message(
        payer: &Pubkey,
        instructions: &[Instruction],
        recent_blockhash: Hash,
    ) -> SanitizedMessage {
        SanitizedMessage::try_new(
            SanitizedVersionedMessage::try_new(VersionedMessage::V0(
                v0::Message::try_compile(payer, instructions, &[], recent_blockhash).unwrap(),
            ))
            .unwrap(),
            SimpleAddressLoader::Enabled(LoadedAddresses::default()),
            &HashSet::new(),
        )
        .unwrap()
    }

    /// Helper: build the Pinocchio Fill instruction.
    #[allow(clippy::too_many_arguments)]
    fn build_fill_ix(
        taker: Pubkey,
        maker: Pubkey,
        input_mint: Pubkey,
        input_token_program: Pubkey,
        output_mint: Pubkey,
        output_token_program: Pubkey,
        taker_input_ta: Option<Pubkey>,
        maker_input_ta: Option<Pubkey>,
        taker_output_ta: Option<Pubkey>,
        maker_output_ta: Option<Pubkey>,
        temp_wsol_ta: Option<Pubkey>,
        input_amount: u128,
        output_amount: u128,
        expire_at: i64,
    ) -> Instruction {
        let mut flags = 0u8;
        if taker_input_ta.is_some() {
            flags |= FLAG_TAKER_INPUT_TA;
        }
        if maker_input_ta.is_some() {
            flags |= FLAG_MAKER_INPUT_TA;
        }
        if taker_output_ta.is_some() {
            flags |= FLAG_TAKER_OUTPUT_TA;
        }
        if maker_output_ta.is_some() {
            flags |= FLAG_MAKER_OUTPUT_TA;
        }
        if temp_wsol_ta.is_some() {
            flags |= FLAG_TEMP_WSOL_TA;
        }
        let mut data = Vec::with_capacity(FILL_DATA_LEN);
        data.push(FILL_DISCRIMINATOR);
        data.push(flags);
        data.extend_from_slice(&input_amount.to_le_bytes());
        data.extend_from_slice(&output_amount.to_le_bytes());
        data.extend_from_slice(&expire_at.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes()); // fee_bps
        let mut accounts = vec![
            AccountMeta::new(taker, true),
            AccountMeta::new(maker, true),
            AccountMeta::new_readonly(input_mint, false),
            AccountMeta::new_readonly(input_token_program, false),
            AccountMeta::new_readonly(output_mint, false),
            AccountMeta::new_readonly(output_token_program, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        for ta in [
            taker_input_ta,
            maker_input_ta,
            taker_output_ta,
            maker_output_ta,
            temp_wsol_ta,
        ]
        .into_iter()
        .flatten()
        {
            accounts.push(AccountMeta::new(ta, false));
        }
        Instruction {
            program_id: ORDER_ENGINE_ID,
            accounts,
            data,
        }
    }

    /// Helper: build a Jupnet TransferChecked (34-byte data: 1 disc + 32 amount LE + 1 decimals).
    fn build_transfer_checked_ix(
        token_program: Pubkey,
        source: Pubkey,
        mint: Pubkey,
        destination: Pubkey,
        authority: Pubkey,
        amount: u128,
        decimals: u8,
    ) -> Instruction {
        let mut data = Vec::with_capacity(TRANSFER_CHECKED_DATA_LEN);
        data.push(TRANSFER_CHECKED_DISCRIMINATOR);
        let mut amount_bytes = [0u8; 32];
        amount_bytes[..16].copy_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&amount_bytes);
        data.push(decimals);
        Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(source, false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new(destination, false),
                AccountMeta::new_readonly(authority, true),
            ],
            data,
        }
    }

    /// Helper: build an ATA CreateIdempotent instruction.
    fn build_ata_create_idempotent_ix(
        funder: Pubkey,
        ata: Pubkey,
        wallet: Pubkey,
        mint: Pubkey,
        token_program: Pubkey,
    ) -> Instruction {
        Instruction {
            program_id: ASSOCIATED_TOKEN_ACCOUNT_ID,
            accounts: vec![
                AccountMeta::new(funder, true),
                AccountMeta::new(ata, false),
                AccountMeta::new_readonly(wallet, false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(token_program, false),
            ],
            data: vec![1],
        }
    }

    fn cu_ixs() -> [Instruction; 2] {
        [
            ComputeBudgetInstruction::set_compute_unit_price(10_000),
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
        ]
    }

    // -----------------------------------------------------------------------
    // validate_fill_sanitized_message
    // -----------------------------------------------------------------------

    #[test]
    fn validate_fill_no_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        let in_amount = 100u128;
        let out_amount = 200u128;
        let expire_at = 1_000;

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            in_amount,
            out_amount,
            expire_at,
        );

        let msg = make_sanitized_message(&maker, &[cu_price, cu_limit, fill], recent_blockhash);
        let validated = validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount,
                input_mint,
                out_amount,
                output_mint,
                expire_at,
                receiver: None,
                output_decimals: 6,
            },
        )
        .unwrap();
        assert_eq!(validated.compute_unit_limit, 200_000);
        assert_eq!(validated.compute_unit_price, 10_000);

        // receiver == taker is treated the same as None.
        validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount,
                input_mint,
                out_amount,
                output_mint,
                expire_at,
                receiver: Some(taker),
                output_decimals: 6,
            },
        )
        .unwrap();
    }

    #[test]
    fn validate_fill_with_native_sol_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        let in_amount = 100u128;
        let out_amount = 200u128;
        let expire_at = 1_000;

        let [cu_price, cu_limit] = cu_ixs();
        // Native SOL output: no taker_output_ta on the Fill ix.
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            NATIVE_MINT,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            Some(Pubkey::new_unique()),
            None,
            in_amount,
            out_amount,
            expire_at,
        );
        let xfer = system_transfer(&taker, &receiver, out_amount as u64);

        let msg = make_sanitized_message(
            &maker,
            &[cu_price, cu_limit, fill, xfer],
            recent_blockhash,
        );
        validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount,
                input_mint,
                out_amount,
                output_mint: NATIVE_MINT,
                expire_at,
                receiver: Some(receiver),
                output_decimals: 9,
            },
        )
        .unwrap();
    }

    #[test]
    fn validate_fill_with_spl_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        let in_amount = 100u128;
        let out_amount = 200u128;
        let expire_at = 1_000;
        let output_decimals = 6;

        let taker_output_ata = derive_ata(&taker, &output_mint, &TOKEN_PROGRAM_ID);
        let receiver_output_ata = derive_ata(&receiver, &output_mint, &TOKEN_PROGRAM_ID);

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(taker_output_ata),
            Some(Pubkey::new_unique()),
            None,
            in_amount,
            out_amount,
            expire_at,
        );
        let create_ata = build_ata_create_idempotent_ix(
            taker, // funder is the taker (NOT the maker)
            receiver_output_ata,
            receiver,
            output_mint,
            TOKEN_PROGRAM_ID,
        );
        let xfer = build_transfer_checked_ix(
            TOKEN_PROGRAM_ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount,
            output_decimals,
        );

        let msg = make_sanitized_message(
            &maker,
            &[cu_price, cu_limit, fill, create_ata, xfer],
            recent_blockhash,
        );
        validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount,
                input_mint,
                out_amount,
                output_mint,
                expire_at,
                receiver: Some(receiver),
                output_decimals,
            },
        )
        .unwrap();
    }

    #[test]
    fn validate_fill_with_third_party_ata_payer() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let ata_payer = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        let in_amount = 100u128;
        let out_amount = 200u128;
        let expire_at = 1_000;
        let output_decimals = 6;

        let taker_output_ata = derive_ata(&taker, &output_mint, &TOKEN_PROGRAM_ID);
        let receiver_output_ata = derive_ata(&receiver, &output_mint, &TOKEN_PROGRAM_ID);

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(taker_output_ata),
            Some(Pubkey::new_unique()),
            None,
            in_amount,
            out_amount,
            expire_at,
        );
        let create_ata = build_ata_create_idempotent_ix(
            ata_payer,
            receiver_output_ata,
            receiver,
            output_mint,
            TOKEN_PROGRAM_ID,
        );
        let xfer = build_transfer_checked_ix(
            TOKEN_PROGRAM_ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount,
            output_decimals,
        );

        let msg = make_sanitized_message(
            &maker,
            &[cu_price, cu_limit, fill, create_ata, xfer],
            recent_blockhash,
        );
        assert_eq!(msg.get_signature_details().num_transaction_signatures(), 3);

        validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount,
                input_mint,
                out_amount,
                output_mint,
                expire_at,
                receiver: Some(receiver),
                output_decimals,
            },
        )
        .unwrap();
    }

    #[test]
    fn rejects_too_many_signers() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();

        let [cu_price, cu_limit] = cu_ixs();
        let mut fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        // Push two extra signers to overflow.
        fill.accounts.push(AccountMeta::new(Pubkey::new_unique(), true));
        fill.accounts.push(AccountMeta::new(Pubkey::new_unique(), true));

        let msg = make_sanitized_message(&maker, &[cu_price, cu_limit, fill], recent_blockhash);
        let err = validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount: 100,
                input_mint,
                out_amount: 200,
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Expected 2 or 3 signers, got 4");
    }

    #[test]
    fn rejects_missing_transfer_when_receiver_set() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );

        let msg = make_sanitized_message(&maker, &[cu_price, cu_limit, fill], recent_blockhash);
        let err = validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount: 100,
                input_mint,
                out_amount: 200,
                output_mint,
                expire_at: 1_000,
                receiver: Some(receiver),
                output_decimals: 6,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Missing transfer instruction for receiver");
    }

    #[test]
    fn rejects_unexpected_transfer() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            NATIVE_MINT,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let xfer = system_transfer(&taker, &receiver, 200);

        let msg = make_sanitized_message(
            &maker,
            &[cu_price, cu_limit, fill, xfer],
            recent_blockhash,
        );
        let err = validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount: 100,
                input_mint,
                out_amount: 200,
                output_mint: NATIVE_MINT,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 9,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Unexpected transfer instruction");
    }

    #[test]
    fn rejects_wrong_transfer_amount() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        let out_amount = 200u128;

        let taker_output_ata = derive_ata(&taker, &output_mint, &TOKEN_PROGRAM_ID);
        let receiver_output_ata = derive_ata(&receiver, &output_mint, &TOKEN_PROGRAM_ID);

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(taker_output_ata),
            Some(Pubkey::new_unique()),
            None,
            100,
            out_amount,
            1_000,
        );
        let xfer = build_transfer_checked_ix(
            TOKEN_PROGRAM_ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount - 1, // wrong amount
            6,
        );

        let msg = make_sanitized_message(
            &maker,
            &[cu_price, cu_limit, fill, xfer],
            recent_blockhash,
        );
        let err = validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount: 100,
                input_mint,
                out_amount,
                output_mint,
                expire_at: 1_000,
                receiver: Some(receiver),
                output_decimals: 6,
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Receiver transfer amount must equal out_amount"
        );
    }

    #[test]
    fn rejects_unrecognised_token_program() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        // Pass the ATA program as the input_token_program.
        let bogus = ASSOCIATED_TOKEN_ACCOUNT_ID;

        let [cu_price, cu_limit] = cu_ixs();
        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            bogus,
            output_mint,
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let msg = make_sanitized_message(&maker, &[cu_price, cu_limit, fill], recent_blockhash);
        let err = validate_fill_sanitized_message(
            &msg,
            Order {
                taker,
                maker,
                in_amount: 100,
                input_mint,
                out_amount: 200,
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Unrecognised input token program");
    }

    // -----------------------------------------------------------------------
    // validate_similar_fill_sanitized_message
    // -----------------------------------------------------------------------

    #[test]
    fn similar_fill_identical_passes() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let recent_blockhash = unique_hash();
        let input_mint = Pubkey::new_unique();
        let taker_input_ta = Pubkey::new_unique();
        let in_amount = 100u128;
        let expire_at = 1000i64;

        let fill = build_fill_ix(
            taker,
            maker,
            input_mint,
            TOKEN_PROGRAM_ID,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Some(taker_input_ta),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            in_amount,
            200,
            expire_at,
        );
        let original = make_sanitized_message(&maker, &[fill.clone()], recent_blockhash);
        let result =
            validate_similar_fill_sanitized_message(original.clone(), original.clone()).unwrap();
        assert_eq!(result.taker, taker);
        assert_eq!(result.input_amount, in_amount);
        assert_eq!(result.input_mint, input_mint);
        assert_eq!(result.taker_input_mint_token_account, Some(taker_input_ta));
        assert_eq!(result.expire_at, expire_at);
    }

    #[test]
    fn similar_fill_different_blockhash_passes() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let blockhash_a = unique_hash();
        let blockhash_b = unique_hash();
        let fill = build_fill_ix(
            taker,
            maker,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let original = make_sanitized_message(&maker, &[fill.clone()], blockhash_a);
        let other = make_sanitized_message(&maker, &[fill], blockhash_b);
        validate_similar_fill_sanitized_message(other, original).unwrap();
    }

    #[test]
    fn similar_fill_rejects_changed_accounts() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let blockhash = unique_hash();
        let fill = build_fill_ix(
            taker,
            maker,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let mut modified = fill.clone();
        // Position 3 is `input_token_program`; mutating it must fail equality.
        modified.accounts[3].pubkey = Pubkey::new_unique();
        let original = make_sanitized_message(&maker, &[fill], blockhash);
        let other = make_sanitized_message(&maker, &[modified], blockhash);
        let err = validate_similar_fill_sanitized_message(other, original).unwrap_err();
        assert!(
            err.to_string()
                .starts_with("Instruction accounts did not match the original"),
            "got: {err}"
        );
    }

    #[test]
    fn similar_fill_rejects_changed_data() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let blockhash = unique_hash();
        let fill = build_fill_ix(
            taker,
            maker,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let mut modified = fill.clone();
        // Flip the high byte of fee_bps (last data byte).
        *modified.data.last_mut().unwrap() = 2;
        let original = make_sanitized_message(&maker, &[fill], blockhash);
        let other = make_sanitized_message(&maker, &[modified], blockhash);
        let err = validate_similar_fill_sanitized_message(other, original).unwrap_err();
        assert!(
            err.to_string()
                .starts_with("Instruction did not match the original"),
            "got: {err}"
        );
    }

    #[test]
    fn similar_fill_compute_budget_can_differ() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let blockhash = unique_hash();
        let fill = build_fill_ix(
            taker,
            maker,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let original_msg = make_sanitized_message(
            &maker,
            &[
                ComputeBudgetInstruction::set_compute_unit_price(10_000),
                ComputeBudgetInstruction::set_compute_unit_limit(200_000),
                fill.clone(),
            ],
            blockhash,
        );
        // Same account list / fill bytes, mutated compute-budget args.
        let other_msg = make_sanitized_message(
            &maker,
            &[
                ComputeBudgetInstruction::set_compute_unit_price(50_000),
                ComputeBudgetInstruction::set_compute_unit_limit(400_000),
                fill,
            ],
            blockhash,
        );
        validate_similar_fill_sanitized_message(other_msg, original_msg).unwrap();
    }

    #[test]
    fn similar_fill_rejects_extra_instruction() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let blockhash = unique_hash();
        let fill = build_fill_ix(
            taker,
            maker,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Pubkey::new_unique(),
            TOKEN_PROGRAM_ID,
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            Some(Pubkey::new_unique()),
            None,
            100,
            200,
            1_000,
        );
        let original_msg = make_sanitized_message(&maker, &[fill.clone()], blockhash);
        let with_extra = make_sanitized_message(
            &maker,
            &[fill, system_transfer(&taker, &Pubkey::new_unique(), 1)],
            blockhash,
        );
        let err = validate_similar_fill_sanitized_message(with_extra, original_msg).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Number of instructions must match the original"
        );
    }
}
