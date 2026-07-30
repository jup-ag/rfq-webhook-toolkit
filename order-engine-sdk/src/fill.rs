use crate::order_engine;
use anchor_lang::{pubkey, AnchorDeserialize, Discriminator};
use anchor_spl::{
    associated_token::{self, get_associated_token_address_with_program_id},
    token::{self, spl_token::instruction::TokenInstruction},
    token_2022,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use solana_sdk::{
    borsh1::try_from_slice_unchecked,
    compute_budget::{self, ComputeBudgetInstruction},
    message::SanitizedMessage,
    pubkey::Pubkey,
    system_instruction::SystemInstruction,
    system_program,
    sysvar::instructions::BorrowedInstruction,
};

const LIGHTHOUSE_PROGRAM_ID: Pubkey = pubkey!("L2TExMFKdjpN9kozasaurPirfHy9P8sbXoAN1qA3S95");
const NATIVE_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

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
    pub receiver: Option<Pubkey>,
    pub output_decimals: u8,
    pub integrator_fee: Option<IntegratorFeeExpectation>,
}

/// Describes the integrator-fee transfer the validator should expect in the fill transaction.
#[derive(Debug, Clone)]
pub struct IntegratorFeeExpectation {
    /// Integrator's token account receiving the fee.
    pub destination: Pubkey,
    /// Exact units transferred to the integrator.
    pub fee_amount: u64,
}

struct FillExtracted {
    taker_output_mint_token_account: Pubkey,
    output_token_program: Pubkey,
}

#[derive(Debug)]
pub struct ValidatedFill {
    pub compute_unit_limit: u32,
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
    let expected_integrator = order.integrator_fee.as_ref();
    let mut fill_ix_found = false;
    let mut receiver_transfer_found = false;
    let mut integrator_fee_validated = false;
    let mut integrator_used_native_path = false;
    let mut integrator_sync_native_validated = false;
    let mut compute_unit_limit = None;
    let mut compute_unit_price = None;
    let mut fill_extracted: Option<FillExtracted> = None;

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
            ensure!(accounts.first().map(|am| am.pubkey) != Some(&order.maker));
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
            let [taker, maker, _taker_input_mint_token_account, _maker_input_mint_token_account, taker_output_mint_token_account, _maker_output_mint_token_account, input_mint, _input_token_program, output_mint, output_token_program, ..] =
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

            fill_extracted = Some(FillExtracted {
                taker_output_mint_token_account: *taker_output_mint_token_account,
                output_token_program: *output_token_program,
            });
        } else if program_id == &system_program::ID {
            let SystemInstruction::Transfer { lamports } = bincode::deserialize(data)
                .map_err(|e| anyhow!("Invalid system instruction: {e}"))?
            else {
                bail!("Unexpected system program instruction");
            };
            let [from, to, ..] = accounts.as_slice() else {
                bail!("Not enough accounts in system transfer");
            };

            ensure!(
                *from.pubkey == order.taker,
                "System transfer source must be taker"
            );

            let is_integrator_transfer = expected_integrator
                .map(|i| *to.pubkey == i.destination)
                .unwrap_or(false);
            if is_integrator_transfer {
                let integrator = expected_integrator.expect("checked just above");
                ensure!(
                    !integrator_fee_validated,
                    "Duplicated integrator-fee transfer"
                );
                ensure!(
                    lamports == integrator.fee_amount,
                    "Integrator-fee transfer amount must equal expected fee"
                );
                integrator_fee_validated = true;
                integrator_used_native_path = true;
            } else {
                ensure!(
                    !receiver_transfer_found,
                    "Duplicated receiver transfer instruction"
                );
                let receiver = expected_receiver.context("Unexpected transfer instruction")?;
                ensure!(
                    order.output_mint == NATIVE_MINT,
                    "Unexpected system_program transfer for non-native output"
                );
                ensure!(
                    *to.pubkey == receiver,
                    "Receiver transfer destination must be the receiver"
                );
                ensure!(
                    lamports == order.out_amount,
                    "Receiver transfer amount must equal out_amount"
                );
                receiver_transfer_found = true;
            }
        } else if program_id == &token::ID || program_id == &token_2022::ID {
            let token_ix = TokenInstruction::unpack(data)
                .map_err(|e| anyhow!("Invalid token instruction: {e}"))?;
            match token_ix {
                TokenInstruction::SyncNative => {
                    let integrator =
                        expected_integrator.context("Unexpected sync_native instruction")?;
                    let [account, ..] = accounts.as_slice() else {
                        bail!("Not enough accounts in sync_native");
                    };
                    ensure!(
                        *account.pubkey == integrator.destination,
                        "sync_native must target the integrator destination"
                    );
                    ensure!(
                        integrator_used_native_path,
                        "sync_native must come after the native-SOL integrator-fee transfer"
                    );
                    ensure!(
                        !integrator_sync_native_validated,
                        "Duplicated sync_native instruction"
                    );
                    integrator_sync_native_validated = true;
                }
                TokenInstruction::TransferChecked { amount, decimals } => {
                    let [source, mint, destination, authority, ..] = accounts.as_slice() else {
                        bail!("Not enough accounts in transfer_checked");
                    };
                    let is_integrator_transfer = expected_integrator
                        .map(|i| *destination.pubkey == i.destination)
                        .unwrap_or(false);
                    if is_integrator_transfer {
                        let integrator = expected_integrator.expect("checked just above");
                        ensure!(
                            !integrator_fee_validated,
                            "Duplicated integrator-fee transfer"
                        );
                        ensure!(
                            *authority.pubkey == order.taker,
                            "Integrator-fee transfer authority must be the taker"
                        );
                        ensure!(
                            amount == integrator.fee_amount,
                            "Integrator-fee transfer amount must equal expected fee"
                        );
                        integrator_fee_validated = true;
                    } else {
                        // Receiver-ATA transfer — existing logic.
                        ensure!(
                            !receiver_transfer_found,
                            "Duplicated receiver transfer instruction"
                        );
                        let receiver =
                            expected_receiver.context("Unexpected transfer instruction")?;
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
                        let expected_destination = get_associated_token_address_with_program_id(
                            &receiver,
                            &order.output_mint,
                            program_id,
                        );
                        ensure!(
                            *source.pubkey == fill.taker_output_mint_token_account,
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
                        receiver_transfer_found = true;
                    }
                }
                _ => {
                    bail!("Only transfer_checked or sync_native are allowed from the token program")
                }
            }
        } else {
            bail!("Unexpected program id {program_id}");
        }
    }

    ensure!(fill_ix_found, "Missing fill instruction");
    ensure!(
        receiver_transfer_found || expected_receiver.is_none(),
        "Missing transfer instruction for receiver"
    );
    if expected_integrator.is_some() {
        ensure!(
            integrator_fee_validated,
            "Missing integrator-fee transfer instruction"
        );
        if integrator_used_native_path {
            ensure!(
                integrator_sync_native_validated,
                "Missing sync_native after native-SOL integrator-fee transfer"
            );
        }
    }

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
        let taker_input_mint_token_account = Pubkey::new_unique();

        let fill_ix = Instruction {
            program_id: order_engine::ID,
            accounts: order_engine::client::accounts::Fill {
                taker,
                maker,
                taker_input_mint_token_account: Some(taker_input_mint_token_account),
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
            taker_input_mint_token_account,
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

    #[allow(clippy::too_many_arguments)]
    fn build_fill_ix(
        taker: Pubkey,
        maker: Pubkey,
        input_mint: Pubkey,
        output_mint: Pubkey,
        taker_output_mint_token_account: Option<Pubkey>,
        output_token_program: Pubkey,
        input_amount: u64,
        output_amount: u64,
        expire_at: i64,
    ) -> Instruction {
        Instruction {
            program_id: order_engine::ID,
            accounts: order_engine::client::accounts::Fill {
                taker,
                maker,
                taker_input_mint_token_account: Some(Pubkey::new_unique()),
                maker_input_mint_token_account: Some(Pubkey::new_unique()),
                taker_output_mint_token_account,
                maker_output_mint_token_account: Some(Pubkey::new_unique()),
                input_mint,
                input_token_program: token::ID,
                output_mint,
                output_token_program,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: order_engine::client::args::Fill {
                input_amount,
                output_amount,
                expire_at,
            }
            .data(),
        }
    }

    fn transfer_checked_ix(
        token_program: Pubkey,
        source: Pubkey,
        mint: Pubkey,
        destination: Pubkey,
        authority: Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Instruction {
        anchor_spl::token::spl_token::instruction::transfer_checked(
            &token_program,
            &source,
            &mint,
            &destination,
            &authority,
            &[],
            amount,
            decimals,
        )
        .unwrap()
    }

    fn cu_ixs() -> [Instruction; 2] {
        [
            ComputeBudgetInstruction::set_compute_unit_price(10_000),
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
        ]
    }

    #[test]
    fn test_validate_fill_no_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix],
            recent_blockhash,
        );

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
                integrator_fee: None,
            },
        )
        .unwrap();
        assert_eq!(validated.compute_unit_limit, 200_000);
        assert_eq!(validated.compute_unit_price, 10_000);

        // Same message but receiver == taker should also pass (treated as no receiver)
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
                integrator_fee: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn test_validate_fill_with_native_sol_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        // Native SOL output: taker_output_mint_token_account is None
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            NATIVE_MINT,
            None,
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );
        let transfer_ix = solana_sdk::system_instruction::transfer(&taker, &receiver, out_amount);

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, transfer_ix],
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
                integrator_fee: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn test_validate_fill_with_spl_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;
        let output_decimals = 6;

        let taker_output_ata =
            get_associated_token_address_with_program_id(&taker, &output_mint, &token::ID);
        let receiver_output_ata =
            get_associated_token_address_with_program_id(&receiver, &output_mint, &token::ID);

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(taker_output_ata),
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );
        let create_ata_ix = Instruction {
            program_id: associated_token::ID,
            accounts: vec![
                AccountMeta::new(taker, true), // funder is the taker (not the maker)
                AccountMeta::new(receiver_output_ata, false),
                AccountMeta::new_readonly(receiver, false),
                AccountMeta::new_readonly(output_mint, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(token::ID, false),
            ],
            data: vec![1],
        };
        let transfer_ix = transfer_checked_ix(
            token::ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount,
            output_decimals,
        );

        let msg = make_sanitized_transaction(
            &maker,
            &[
                cu_price_ix,
                cu_limit_ix,
                fill_ix,
                create_ata_ix,
                transfer_ix,
            ],
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
                integrator_fee: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn test_validate_fill_with_third_party_ata_payer() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let ata_payer = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;
        let output_decimals = 6;

        let taker_output_ata =
            get_associated_token_address_with_program_id(&taker, &output_mint, &token::ID);
        let receiver_output_ata =
            get_associated_token_address_with_program_id(&receiver, &output_mint, &token::ID);

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(taker_output_ata),
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );
        let create_ata_ix = Instruction {
            program_id: associated_token::ID,
            accounts: vec![
                AccountMeta::new(ata_payer, true),
                AccountMeta::new(receiver_output_ata, false),
                AccountMeta::new_readonly(receiver, false),
                AccountMeta::new_readonly(output_mint, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(token::ID, false),
            ],
            data: vec![1],
        };
        let transfer_ix = transfer_checked_ix(
            token::ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount,
            output_decimals,
        );

        let msg = make_sanitized_transaction(
            &maker,
            &[
                cu_price_ix,
                cu_limit_ix,
                fill_ix,
                create_ata_ix,
                transfer_ix,
            ],
            recent_blockhash,
        );

        // Sanity check: this transaction has 3 signers
        assert_eq!(msg.get_signature_details().num_transaction_signatures(), 3);
        assert_eq!(msg.account_keys().get(0), Some(&maker));
        let signers = msg.account_keys().iter().take(3).collect::<Vec<_>>();
        assert!(signers.contains(&&taker));
        assert!(signers.contains(&&ata_payer));

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
                integrator_fee: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn test_validate_fill_rejects_too_many_signers() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let extra_signer = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let mut fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );
        // Inject a third signer
        fill_ix.accounts.push(AccountMeta {
            pubkey: extra_signer,
            is_signer: true,
            is_writable: true,
        });
        // Inject a fourth signer to push the total over the limit
        fill_ix.accounts.push(AccountMeta {
            pubkey: Pubkey::new_unique(),
            is_signer: true,
            is_writable: true,
        });

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix],
            recent_blockhash,
        );

        let err = validate_fill_sanitized_message(
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
                integrator_fee: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Expected 2 or 3 signers, got 4");
    }

    #[test]
    fn test_validate_fill_rejects_missing_transfer_when_receiver_set() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix],
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
                output_mint,
                expire_at: 1_000,
                receiver: Some(receiver),
                output_decimals: 6,
                integrator_fee: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Missing transfer instruction for receiver");
    }

    #[test]
    fn test_validate_fill_rejects_unexpected_transfer() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            NATIVE_MINT,
            None,
            token::ID,
            100,
            200,
            1_000,
        );
        let transfer_ix = solana_sdk::system_instruction::transfer(&taker, &receiver, 200);

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, transfer_ix],
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
                integrator_fee: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Unexpected transfer instruction");
    }

    #[test]
    fn test_validate_fill_rejects_wrong_transfer_amount() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let out_amount = 200;

        let taker_output_ata =
            get_associated_token_address_with_program_id(&taker, &output_mint, &token::ID);
        let receiver_output_ata =
            get_associated_token_address_with_program_id(&receiver, &output_mint, &token::ID);

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(taker_output_ata),
            token::ID,
            100,
            out_amount,
            1_000,
        );
        // Transfer the wrong amount (out_amount - 1)
        let transfer_ix = transfer_checked_ix(
            token::ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount - 1,
            6,
        );

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, transfer_ix],
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
                integrator_fee: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Receiver transfer amount must equal out_amount"
        );
    }

    // ── integrator-fee validation ────────────────────────────────────────────────────────

    fn sync_native_ix(account: Pubkey) -> Instruction {
        anchor_spl::token::spl_token::instruction::sync_native(&token::ID, &account).unwrap()
    }

    fn system_transfer_ix(from: Pubkey, to: Pubkey, lamports: u64) -> Instruction {
        solana_sdk::system_instruction::transfer(&from, &to, lamports)
    }

    #[test]
    fn test_validate_fill_with_native_integrator_fee() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;
        let integrator_destination = Pubkey::new_unique();
        let fee_amount = 1_000;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );
        let fee_transfer = system_transfer_ix(taker, integrator_destination, fee_amount);
        let sync_native = sync_native_ix(integrator_destination);

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, fee_transfer, sync_native],
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
                receiver: None,
                output_decimals: 6,
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount,
                }),
            },
        )
        .expect("native-SOL integrator fee + sync_native must validate");
    }

    #[test]
    fn test_validate_fill_with_spl_integrator_fee_and_receiver() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let receiver = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let in_amount = 100;
        let out_amount = 200;
        let expire_at = 1_000;
        let output_decimals = 6;
        let taker_output_ata = Pubkey::new_unique();
        let receiver_output_ata =
            get_associated_token_address_with_program_id(&receiver, &output_mint, &token::ID);
        let taker_input_ata = Pubkey::new_unique();
        let integrator_destination = Pubkey::new_unique();
        let fee_amount = 50;
        let fee_decimals = 6;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(taker_output_ata),
            token::ID,
            in_amount,
            out_amount,
            expire_at,
        );
        let integrator_transfer = transfer_checked_ix(
            token::ID,
            taker_input_ata,
            input_mint,
            integrator_destination,
            taker,
            fee_amount,
            fee_decimals,
        );
        let receiver_transfer = transfer_checked_ix(
            token::ID,
            taker_output_ata,
            output_mint,
            receiver_output_ata,
            taker,
            out_amount,
            output_decimals,
        );

        let msg = make_sanitized_transaction(
            &maker,
            &[
                cu_price_ix,
                cu_limit_ix,
                fill_ix,
                integrator_transfer,
                receiver_transfer,
            ],
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
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount,
                }),
            },
        )
        .expect("SPL integrator fee + receiver transfer must validate");
    }

    #[test]
    fn test_validate_fill_rejects_missing_integrator_fee_transfer() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let integrator_destination = Pubkey::new_unique();

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );
        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix],
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
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount: 1_000,
                }),
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Missing integrator-fee transfer instruction"
        );
    }

    #[test]
    fn test_validate_fill_rejects_missing_sync_native() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let integrator_destination = Pubkey::new_unique();
        let fee_amount = 1_000;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );
        let fee_transfer = system_transfer_ix(taker, integrator_destination, fee_amount);

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, fee_transfer],
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
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount,
                }),
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Missing sync_native after native-SOL integrator-fee transfer"
        );
    }

    #[test]
    fn test_validate_fill_rejects_wrong_integrator_fee_amount() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let integrator_destination = Pubkey::new_unique();
        let expected_fee = 1_000;
        let actual_lamports = 999;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );
        let fee_transfer = system_transfer_ix(taker, integrator_destination, actual_lamports);
        let sync_native = sync_native_ix(integrator_destination);

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, fee_transfer, sync_native],
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
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount: expected_fee,
                }),
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Integrator-fee transfer amount must equal expected fee"
        );
    }

    #[test]
    fn test_validate_fill_rejects_unexpected_sync_native() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let rogue_account = Pubkey::new_unique();

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );
        let sync_native = sync_native_ix(rogue_account);

        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, sync_native],
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
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
                integrator_fee: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Unexpected sync_native instruction");
    }

    #[test]
    fn test_validate_fill_rejects_sync_native_before_native_transfer() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let integrator_destination = Pubkey::new_unique();
        let fee_amount = 1_000;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );
        let fee_transfer = system_transfer_ix(taker, integrator_destination, fee_amount);
        let sync_native = sync_native_ix(integrator_destination);

        // sync_native FIRST, then the system transfer.
        let msg = make_sanitized_transaction(
            &maker,
            &[cu_price_ix, cu_limit_ix, fill_ix, sync_native, fee_transfer],
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
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount,
                }),
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "sync_native must come after the native-SOL integrator-fee transfer"
        );
    }

    #[test]
    fn test_validate_fill_rejects_sync_native_with_spl_path() {
        let taker = Pubkey::new_unique();
        let maker = Pubkey::new_unique();
        let input_mint = Pubkey::new_unique();
        let output_mint = Pubkey::new_unique();
        let recent_blockhash = Hash::new_unique();
        let taker_input_ata = Pubkey::new_unique();
        let integrator_destination = Pubkey::new_unique();
        let fee_amount = 50;
        let fee_decimals = 6;

        let [cu_price_ix, cu_limit_ix] = cu_ixs();
        let fill_ix = build_fill_ix(
            taker,
            maker,
            input_mint,
            output_mint,
            Some(Pubkey::new_unique()),
            token::ID,
            100,
            200,
            1_000,
        );
        let integrator_transfer = transfer_checked_ix(
            token::ID,
            taker_input_ata,
            input_mint,
            integrator_destination,
            taker,
            fee_amount,
            fee_decimals,
        );
        let sync_native = sync_native_ix(integrator_destination);

        let msg = make_sanitized_transaction(
            &maker,
            &[
                cu_price_ix,
                cu_limit_ix,
                fill_ix,
                integrator_transfer,
                sync_native,
            ],
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
                output_mint,
                expire_at: 1_000,
                receiver: None,
                output_decimals: 6,
                integrator_fee: Some(IntegratorFeeExpectation {
                    destination: integrator_destination,
                    fee_amount,
                }),
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "sync_native must come after the native-SOL integrator-fee transfer"
        );
    }
}
