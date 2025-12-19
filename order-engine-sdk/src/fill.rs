use crate::order_engine;
use anchor_lang::{pubkey, AnchorDeserialize, Discriminator};
use anchor_spl::associated_token;
use anyhow::{anyhow, bail, ensure, Context, Result};
use solana_sdk::{
    borsh1::try_from_slice_unchecked,
    compute_budget::{self, ComputeBudgetInstruction},
    message::SanitizedMessage,
    pubkey::Pubkey,
    sysvar::instructions::BorrowedInstruction,
};

const LIGHTHOUSE_PROGRAM_ID: Pubkey = pubkey!("L2TExMFKdjpN9kozasaurPirfHy9P8sbXoAN1qA3S95");

// We only allow certain instruction from the Lighthouse program.
//
// If we allow the MemoryWrite instruction, the hacker can drain the signer.
// https://github.com/Jac0xb/lighthouse/blob/main/programs/lighthouse/lighthouse.json
const ALLOWED_LIGHTHOUSE_DISCRIMINATORS: &[u8] = &[5, 6, 9, 10];

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
            let [taker, maker, _taker_input_mint_token_account, _maker_input_mint_token_account, _taker_output_mint_token_account, _maker_output_mint_token_account, input_mint, _input_token_program, output_mint, _output_mint_token_program, ..] =
                pubkeys.as_slice()
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

#[derive(PartialEq, Debug)]
pub struct ValidatedSimilarFill {
    pub taker: Pubkey,
    pub input_amount: u64,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub taker_input_mint_token_account: Pubkey,
    pub expire_at: i64,
}

/// Given the original sanitized message, allow some minor changes
pub fn validate_similar_fill_sanitized_message(
    sanitized_message: SanitizedMessage,
    original_sanitized_message: SanitizedMessage,
) -> Result<ValidatedSimilarFill> {
    let message_header = sanitized_message.header();
    let original_message_header = original_sanitized_message.header();

    ensure!(
        original_message_header.num_required_signatures == message_header.num_required_signatures,
        "Number of required signatures did not match"
    );
    let mut account_keys_iter = sanitized_message.account_keys().iter();
    for original_signer in original_sanitized_message
        .account_keys()
        .iter()
        .take(usize::from(original_message_header.num_required_signatures))
    {
        let signer = account_keys_iter
            .next()
            .context("Not enough account keys to validate signer")?;
        ensure!(signer == original_signer, "Signer did not match");
    }

    let sanitized_instructions = sanitized_message.decompile_instructions();
    let original_instructions = original_sanitized_message.decompile_instructions();

    // Validate that we have at least the original number of instructions
    ensure!(
        sanitized_instructions.len() >= original_instructions.len(),
        "Number of instructions cannot be less than original"
    );

    let mut validated_similar_fill = None;
    let mut compute_unit_price = None;
    let mut compute_unit_limit = None;

    // First check matching instructions between original and sanitized
    let mut sanitized_instructions_iter = sanitized_instructions.into_iter();
    let original_len = original_instructions.len();

    for (
        index,
        BorrowedInstruction {
            program_id: original_program_id,
            accounts: original_accounts,
            data: original_data,
        },
    ) in original_instructions.into_iter().enumerate()
    {
        let BorrowedInstruction {
            program_id,
            accounts,
            data,
        } = sanitized_instructions_iter
            .next()
            .context("Missing instruction")?;
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
            // Allow for compute unit price and limit to change, since some wallets change it
            let compute_budget_ix = try_from_slice_unchecked::<ComputeBudgetInstruction>(data)?;
            match compute_budget_ix {
                ComputeBudgetInstruction::SetComputeUnitLimit(limit) => {
                    ensure!(
                        compute_unit_limit.is_none(),
                        "Compute unit limit is already set"
                    );
                    compute_unit_limit = Some(limit);
                    continue;
                }
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
            let output_mint = accounts.get(8).context("Invalid fill ix data")?.pubkey;

            let taker_input_mint_token_account = accounts
                .get(2)
                .context("Invalid taker input mint token account ix data")?
                .pubkey;

            validated_similar_fill = Some(ValidatedSimilarFill {
                taker: *taker,
                input_amount: fill_ix.input_amount,
                input_mint: *input_mint,
                output_mint: *output_mint,
                taker_input_mint_token_account: *taker_input_mint_token_account,
                expire_at: fill_ix.expire_at,
            })
        }
    }

    // Check any additional instructions in sanitized_instructions
    for (
        index,
        BorrowedInstruction {
            program_id,
            accounts: _,
            data,
        },
    ) in sanitized_instructions_iter.enumerate()
    {
        let real_index = index + original_len;
        ensure!(
            program_id == &LIGHTHOUSE_PROGRAM_ID,
            "Additional instructions can only be from Lighthouse program at {real_index}"
        );

        ensure!(
            data.first()
                .map(|discriminator| ALLOWED_LIGHTHOUSE_DISCRIMINATORS.contains(discriminator))
                .unwrap_or(false),
            "Invalid Lighthouse instruction discriminator at index {real_index}"
        );
    }

    validated_similar_fill.context("Missing validated fill instruction")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use anchor_lang::{prelude::*, InstructionData, ToAccountMetas};
    use solana_sdk::{
        hash::Hash,
        instruction::Instruction,
        message::{
            v0::{self, LoadedAddresses},
            SanitizedVersionedMessage, SimpleAddressLoader, VersionedMessage,
        },
        system_program,
    };

    fn make_sanitized_transaction(
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

    #[test]
    fn test_validate_similar_fill_sanitized_message() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let input_amount = 100;
        let expire_at = 1000;
        let input_mint = Pubkey::new_unique();
        let taker_input_mint_token_account = Some(Pubkey::new_unique());

        let fill_ix = Instruction {
            program_id: order_engine::ID,
            accounts: order_engine::client::accounts::Fill {
                taker,
                maker,
                taker_input_mint_token_account,
                maker_input_mint_token_account: Some(Pubkey::new_unique()),
                taker_output_mint_token_account: Some(Pubkey::new_unique()),
                maker_output_mint_token_account: Some(Pubkey::new_unique()),
                input_mint,
                input_token_program: Pubkey::new_unique(),
                output_mint: Pubkey::new_unique(),
                output_token_program: Pubkey::new_unique(),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: order_engine::client::args::Fill {
                input_amount,
                output_amount: 200,
                expire_at,
            }
            .data(),
        };

        let original_sanitized_message =
            make_sanitized_transaction(&maker, &[fill_ix.clone()], recent_blockhash);

        let expected_validated_similar_fill = ValidatedSimilarFill {
            taker,
            input_amount,
            input_mint,
            output_mint: fill_ix.accounts[8].pubkey,
            taker_input_mint_token_account: taker_input_mint_token_account
                .unwrap_or(order_engine::ID),
            expire_at,
        };

        // Identical message
        assert_eq!(
            expected_validated_similar_fill,
            validate_similar_fill_sanitized_message(
                original_sanitized_message.clone(),
                original_sanitized_message.clone()
            )
            .unwrap()
        );

        // Other blockhash
        let sanitized_message =
            make_sanitized_transaction(&maker, &[fill_ix.clone()], Hash::new_unique());
        assert_eq!(
            expected_validated_similar_fill,
            validate_similar_fill_sanitized_message(
                sanitized_message,
                original_sanitized_message.clone()
            )
            .unwrap()
        );

        // Different number of required signatures
        let different_signature_fill_ix = Instruction {
            program_id: order_engine::ID,
            accounts: vec![AccountMeta {
                pubkey: taker,
                is_signer: false,
                is_writable: false,
            }],
            data: order_engine::client::args::Fill {
                input_amount,
                output_amount: 200,
                expire_at,
            }
            .data(),
        };
        let sanitized_message = make_sanitized_transaction(
            &maker,
            &[different_signature_fill_ix.clone()],
            Hash::new_unique(),
        );
        assert_eq!(
            "Number of required signatures did not match",
            validate_similar_fill_sanitized_message(
                sanitized_message,
                original_sanitized_message.clone()
            )
            .unwrap_err()
            .to_string()
        );

        // Change accounts
        let mut modified_fill_ix = fill_ix.clone();
        modified_fill_ix.accounts[3].pubkey = Pubkey::new_unique();
        let sanitized_message =
            make_sanitized_transaction(&maker, &[modified_fill_ix], recent_blockhash);
        assert_eq!(
            "Instruction accounts did not match the original message 0, 61DFfeTKM7trxYcPQCM78bJ794ddZprZpAwAnLiwTpYH",
            validate_similar_fill_sanitized_message(
                sanitized_message,
                original_sanitized_message.clone()
            )
            .unwrap_err()
            .to_string()
        );

        // Change data
        let mut modified_fill_ix = fill_ix.clone();
        *modified_fill_ix.data.last_mut().unwrap() = 2;
        let sanitized_message =
            make_sanitized_transaction(&maker, &[modified_fill_ix], recent_blockhash);
        assert_eq!(
            "Instruction did not match the original at index 0, 61DFfeTKM7trxYcPQCM78bJ794ddZprZpAwAnLiwTpYH",
            validate_similar_fill_sanitized_message(
                sanitized_message,
                original_sanitized_message.clone()
            )
            .unwrap_err()
            .to_string()
        );

        // Add lighthouse instruction
        let lighthouse_ix = Instruction {
            program_id: LIGHTHOUSE_PROGRAM_ID,
            accounts: vec![AccountMeta::new_readonly(input_mint, false)],
            data: vec![5],
        };
        let sanitized_message = make_sanitized_transaction(
            &maker,
            &[fill_ix.clone(), lighthouse_ix.clone()],
            recent_blockhash,
        );
        assert_eq!(
            expected_validated_similar_fill,
            validate_similar_fill_sanitized_message(
                sanitized_message,
                original_sanitized_message.clone()
            )
            .unwrap()
        );

        // Add forbidden lighthouse instruction
        let mut forbidden_lighthouse_ix = lighthouse_ix.clone();
        forbidden_lighthouse_ix.data[0] = 1;
        let sanitized_message = make_sanitized_transaction(
            &maker,
            &[fill_ix.clone(), forbidden_lighthouse_ix],
            recent_blockhash,
        );
        assert_eq!(
            "Invalid Lighthouse instruction discriminator at index 1",
            validate_similar_fill_sanitized_message(
                sanitized_message,
                original_sanitized_message.clone()
            )
            .unwrap_err()
            .to_string()
        );
    }
}
