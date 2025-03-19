use crate::order_engine;
use anchor_lang::{AnchorDeserialize, Discriminator};
use anchor_spl::associated_token;
use anyhow::{Context, Result, anyhow, bail, ensure};
use solana_sdk::{
    borsh1::try_from_slice_unchecked,
    compute_budget::{self, ComputeBudgetInstruction},
    message::SanitizedMessage,
    pubkey::Pubkey,
    sysvar::instructions::BorrowedInstruction,
};

pub struct Order {
    pub taker: Pubkey,
    pub maker: Pubkey,
    pub in_amount: u64,
    pub input_mint: Pubkey,
    pub out_amount: u64,
    pub output_mint: Pubkey,
    pub expire_at: i64,
}

pub struct ValidatedFill {
    pub compute_unit_limit: u32,
    /// The maker should verify that the trade is still viable should the compute unit price change drastically
    /// The compute unit price might change from the original tx as wallets tend to mutate it
    pub compute_unit_price: u64,
}

/// Given the knowledge of the order, validate the fill transaction
pub fn validate_fill_sanitized_message(
    sanitized_message: &SanitizedMessage,
    order: Order,
) -> Result<ValidatedFill> {
    let fee_payer = sanitized_message.fee_payer();
    ensure!(
        fee_payer == &order.maker,
        "Fee payer was not the expected maker {fee_payer} but was {}",
        order.maker,
    );

    ensure!(
        sanitized_message
            .get_signature_details()
            .num_transaction_signatures()
            == 2,
        "Too many signers"
    );

    let second_signer = sanitized_message
        .account_keys()
        .get(1)
        .context("Missing taker")?;
    ensure!(
        second_signer == &order.taker,
        "Second transaction signer is not the taker"
    );

    let mut fill_ix_found = false;
    let mut compute_unit_limit = None;
    let mut compute_unit_price = None;

    for BorrowedInstruction {
        program_id,
        accounts,
        data,
    } in sanitized_message.decompile_instructions()
    {
        if program_id == &compute_budget::ID {
            // Compute budget should have been driven from the fee payer, certainly need to validate
            let compute_budget_ix = try_from_slice_unchecked::<ComputeBudgetInstruction>(data)?;
            match compute_budget_ix {
                ComputeBudgetInstruction::SetComputeUnitLimit(limit) => {
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
        } else if program_id == &associated_token::ID {
            // For simplicity we only allow create ata idempotent
            ensure!(
                data == vec![1],
                "Incorrect associated token account program data"
            );

            // We verify the taker is paying for the token account
            ensure!(accounts.first().map(|am| am.pubkey) == Some(&order.taker));
        } else if program_id == &order_engine::ID {
            ensure!(!fill_ix_found, "Duplicated fill instruction");
            fill_ix_found = true;

            ensure!(data.len() >= 8, "Not enough data in fill instruction");
            // Must slice off anchor's discriminator first
            let (discriminator, mut ix_data) = data.split_at(8);
            ensure!(
                discriminator == order_engine::client::args::Fill::DISCRIMINATOR,
                "Not a fill discriminator"
            );

            let pubkeys = accounts.into_iter().map(|a| *a.pubkey).collect::<Vec<_>>();
            let [
                taker,
                maker,
                _taker_input_mint_token_account,
                _maker_input_mint_token_account,
                _taker_output_mint_token_account,
                _maker_output_mint_token_account,
                input_mint,
                _input_token_program,
                output_mint,
                _output_mint_token_program,
                ..,
            ] = pubkeys.as_slice()
            else {
                bail!("Not enough accounts");
            };

            // Note: The validation isn't total as we don't validate native sol against native mint expectation
            ensure!(taker == &order.taker, "Invalid taker");
            ensure!(maker == &order.maker, "Invalid maker");
            ensure!(input_mint == &order.input_mint, "Invalid input mint");
            ensure!(output_mint == &order.output_mint, "Invalid output mint");

            let fill_ix = order_engine::client::args::Fill::deserialize(&mut ix_data)
                .map_err(|e| anyhow!("Invalid fill ix data {e}"))?;

            // Check the input and output amount
            if fill_ix.input_amount != order.in_amount || fill_ix.output_amount != order.out_amount
            {
                bail!("Invalid fill ix");
            }

            // Check the expiry
            ensure!(fill_ix.expire_at == order.expire_at, "Incorrect expiry");
        } else {
            bail!("Unexpected program id {program_id}");
        }
    }

    ensure!(fill_ix_found, "Missing fill instruction");
    Ok(ValidatedFill {
        compute_unit_limit: compute_unit_limit.context("Missing compute unit limit")?,
        compute_unit_price: compute_unit_price.context("Missing compute unit price")?,
    })
}

pub struct ValidatedSimilarFill {
    pub taker: Pubkey,
    pub input_amount: u64,
    pub input_mint: Pubkey,
    pub taker_input_mint_token_account: Pubkey,
    pub expire_at: i64,
}

/// Given the original sanitized message, allow some minor changes
pub fn validate_similar_fill_sanitized_message(
    sanitized_message: SanitizedMessage,
    original_sanitized_message: SanitizedMessage,
) -> Result<ValidatedSimilarFill> {
    ensure!(
        original_sanitized_message.recent_blockhash() == sanitized_message.recent_blockhash(),
        "Recent blockhash has been modified"
    );

    let original_message_header = original_sanitized_message.header();
    ensure!(
        sanitized_message.header() == original_message_header,
        "Message header has been modified"
    );

    for (original_signer, signer) in original_sanitized_message
        .account_keys()
        .iter()
        .zip(sanitized_message.account_keys().iter())
        .take(usize::from(original_message_header.num_required_signatures))
    {
        ensure!(signer == original_signer, "Signer did not match");
    }

    let sanitized_instructions = sanitized_message.decompile_instructions();
    let original_instructions = original_sanitized_message.decompile_instructions();

    // Validate that we have the same number of instructions
    ensure!(
        sanitized_instructions.len() == original_instructions.len(),
        "Number of instructions did not match"
    );

    let mut validated_similar_fill = None;
    let mut compute_unit_price = None;
    for (
        index,
        (
            BorrowedInstruction {
                program_id,
                accounts,
                data,
            },
            BorrowedInstruction {
                program_id: original_program_id,
                accounts: original_accounts,
                data: original_data,
            },
        ),
    ) in sanitized_instructions
        .into_iter()
        .zip(original_instructions)
        .enumerate()
    {
        ensure!(
            program_id == original_program_id,
            "Instruction program id did not match the original message at index {index}, {original_program_id}"
        );
        ensure!(
            accounts.len() == original_accounts.len(),
            "Instruction accounts length was not equal at index {index}, {original_program_id}"
        );
        ensure!(
            accounts
                .iter()
                .zip(original_accounts)
                .all(|(accounts, original_accounts)| {
                    accounts.pubkey == original_accounts.pubkey
                        && accounts.is_signer == original_accounts.is_signer
                        && accounts.is_writable == original_accounts.is_writable
                }),
            "Instruction accounts did not match the original message {index}, {original_program_id}"
        );
        if original_program_id == &compute_budget::ID {
            // Allow for compute unit price to change, since some wallets change it
            let compute_budget_ix = try_from_slice_unchecked::<ComputeBudgetInstruction>(data)?;
            match compute_budget_ix {
                ComputeBudgetInstruction::SetComputeUnitLimit(_) => (),
                ComputeBudgetInstruction::SetComputeUnitPrice(price) => {
                    ensure!(
                        compute_unit_price.is_none(),
                        "Comput unit price is already set"
                    );
                    compute_unit_price = Some(price);
                    continue;
                }
                _ => bail!("Unexpected compute budget instruction"),
            }
        }

        ensure!(
            data == original_data,
            "Instruction did not match the original at index {index}, {original_program_id}"
        );

        // If the program_id is order_engine then we give additional information to verify
        if program_id == &order_engine::ID {
            ensure!(
                validated_similar_fill.is_none(),
                "Duplicated fill instruction"
            );
            ensure!(data.len() >= 8, "Not enough data in fill instruction");
            let (discriminator, mut ix_data) = data.split_at(8);
            ensure!(
                discriminator == order_engine::client::args::Fill::DISCRIMINATOR,
                "Not a fill discriminator"
            );

            let fill_ix = order_engine::client::args::Fill::deserialize(&mut ix_data)
                .map_err(|e| anyhow!("Invalid fill ix data {e}"))?;
            // We check if the taker has enough balance to fill the order first
            let taker = accounts.first().context("Invalid fill ix data")?.pubkey;
            let input_mint = accounts.get(6).context("Invalid fill ix data")?.pubkey;

            let taker_input_mint_token_account = accounts
                .get(2)
                .context("Invalid taker input mint token account ix data")?
                .pubkey;

            validated_similar_fill = Some(ValidatedSimilarFill {
                taker: *taker,
                input_amount: fill_ix.input_amount,
                input_mint: *input_mint,
                taker_input_mint_token_account: *taker_input_mint_token_account,
                expire_at: fill_ix.expire_at,
            })
        }
    }

    validated_similar_fill.context("Missing validated fill instruction")
}
