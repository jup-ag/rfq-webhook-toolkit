use std::path::PathBuf;

use anchor_lang::{prelude::*, system_program, InstructionData};
use assert_matches::assert_matches;
use itertools::Itertools;
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_account::Account;
use solana_instruction::{error::InstructionError, Instruction};
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_program_pack::Pack;
use solana_signer::Signer;
use solana_system_interface::instruction as system_instruction;
use solana_transaction::Transaction;
use solana_transaction_error::TransactionError;
use spl_associated_token_account_interface::{
    address::get_associated_token_address_with_program_id,
    instruction::create_associated_token_account,
};
use spl_token_2022_interface::{
    self as spl_token_2022,
    extension::{transfer_fee, ExtensionType, StateWithExtensions},
};
use spl_token_interface as spl_token;
use test_case::test_case;

fn get_amount_or_lamports(svm: &LiteSVM, user: Pubkey, token_account: &Option<Pubkey>) -> u64 {
    match token_account {
        Some(token_account) => token_account_amount(svm, token_account),
        None => svm.get_account(&user).unwrap().lamports,
    }
}

#[test_case(Default::default())]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::NativeSol, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::NativeSol, output: AccountKind::Token }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeSol }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeSol }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::NativeSol, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::NativeMint, output: AccountKind::Token }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeSol }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeMint }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::NativeMint, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::NativeSol, output: AccountKind::Token }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeMint }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeSol }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::NativeMint, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::NativeMint, output: AccountKind::Token }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeMint }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::NativeMint }, ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, input_mint_extensions: Some(vec![ExtensionInitializationParams::TransferFeeConfig { transfer_fee_config_authority: None, withdraw_withheld_authority: None, transfer_fee_basis_points: 0, maximum_fee: 0 }]), ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, output_mint_extensions: Some(vec![ExtensionInitializationParams::TransferFeeConfig { transfer_fee_config_authority: None, withdraw_withheld_authority: None, transfer_fee_basis_points: 0, maximum_fee: 0 }]), ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, input_mint_extensions: Some(vec![ExtensionInitializationParams::TransferFeeConfig { transfer_fee_config_authority: None, withdraw_withheld_authority: None, transfer_fee_basis_points: 100, maximum_fee: u64::MAX }]), expected_error: Some(TransactionError::InstructionError(0, InstructionError::Custom(u32::from(order_engine::error::OrderEngineError::Token2022MintExtensionNotSupported)))), ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, input_mint_extensions: Some(vec![ExtensionInitializationParams::NonTransferable]), expected_error: Some(TransactionError::InstructionError(0, InstructionError::Custom(spl_token_2022::error::TokenError::NonTransferable as u32))), ..Default::default()})]
fn test_fill(test_mode: TestMode) {
    let expected_error = test_mode.expected_error.clone();
    let mut test_environment = prepare_test(test_mode);

    let fill_instruction = test_environment.create_fill_instruction();
    let TestEnvironment {
        svm,
        payer,
        taker_keypair,
        maker_keypair,
        input_amount,
        output_amount,
        maker,
        taker,
        taker_input_mint_token_account,
        maker_input_mint_token_account,
        taker_output_mint_token_account,
        maker_output_mint_token_account,
        ..
    } = &mut test_environment;

    let before_taker_balance = get_amount_or_lamports(svm, *taker, &None);
    let before_taker_input_amount =
        get_amount_or_lamports(svm, *taker, taker_input_mint_token_account);
    let before_taker_output_amount =
        get_amount_or_lamports(svm, *taker, taker_output_mint_token_account);

    let before_maker_balance = get_amount_or_lamports(svm, *maker, &None);
    let before_maker_input_amount =
        get_amount_or_lamports(svm, *maker, maker_input_mint_token_account);
    let before_maker_output_amount =
        get_amount_or_lamports(svm, *maker, maker_output_mint_token_account);

    // Maker fills.
    let result = process_instructions(
        &[fill_instruction],
        payer,
        &[taker_keypair, maker_keypair],
        svm,
    );

    match expected_error {
        Some(expected_error) => {
            let FailedTransactionMetadata {
                err: transaction_error,
                ..
            } = result.unwrap_err();
            assert_eq!(transaction_error, expected_error);
            return;
        }
        None => {
            assert_matches!(result, Ok(_));
        }
    }

    let after_taker_balance = get_amount_or_lamports(svm, *taker, &None);
    let after_taker_input_amount =
        get_amount_or_lamports(svm, *taker, taker_input_mint_token_account);
    let after_taker_output_amount =
        get_amount_or_lamports(svm, *taker, taker_output_mint_token_account);

    let after_maker_balance = get_amount_or_lamports(svm, *maker, &None);
    let after_maker_input_amount =
        get_amount_or_lamports(svm, *maker, maker_input_mint_token_account);
    let after_maker_output_amount =
        get_amount_or_lamports(svm, *maker, maker_output_mint_token_account);

    assert_eq!(
        before_taker_input_amount.checked_sub(after_taker_input_amount),
        Some(*input_amount)
    );
    assert_eq!(
        after_taker_output_amount.checked_sub(before_taker_output_amount),
        Some(*output_amount)
    );

    // The native sol balance is not already checked so assert no change.
    if taker_input_mint_token_account.is_none() && taker_output_mint_token_account.is_none() {
        assert_eq!(before_taker_balance, after_taker_balance);
    }

    println!("{after_maker_input_amount} {before_maker_input_amount}");
    assert_eq!(
        after_maker_input_amount.checked_sub(before_maker_input_amount),
        Some(*input_amount)
    );
    println!("{before_maker_output_amount:?} {after_maker_output_amount:?}");
    assert_eq!(
        before_maker_output_amount.checked_sub(after_maker_output_amount),
        Some(*output_amount)
    );

    // The native sol balance is not already checked so assert no change.
    if maker_input_mint_token_account.is_none() && maker_output_mint_token_account.is_none() {
        assert_eq!(before_maker_balance, after_maker_balance);
    }
}

struct TestEnvironment {
    svm: LiteSVM,
    payer: Keypair,
    taker_keypair: Keypair,
    maker_keypair: Keypair,
    input_amount: u64,
    output_amount: u64,
    // accounts
    maker: Pubkey,
    taker: Pubkey,
    taker_input_mint_token_account: Option<Pubkey>,
    maker_input_mint_token_account: Option<Pubkey>,
    taker_output_mint_token_account: Option<Pubkey>,
    maker_output_mint_token_account: Option<Pubkey>,
    input_mint: Pubkey,
    input_token_program: Pubkey,
    output_mint: Pubkey,
    output_token_program: Pubkey,
    temporary_wsol_token_account: Option<Pubkey>,
}

impl TestEnvironment {
    fn create_fill_instruction(&self) -> Instruction {
        let TestEnvironment {
            input_amount,
            output_amount,
            maker,
            taker,
            taker_input_mint_token_account,
            maker_input_mint_token_account,
            taker_output_mint_token_account,
            maker_output_mint_token_account,
            input_mint,
            input_token_program,
            output_mint,
            output_token_program,
            temporary_wsol_token_account,
            ..
        } = self;
        let mut data = order_engine::instruction::Fill {
            input_amount: *input_amount,
            output_amount: *output_amount,
            expire_at: i64::MAX,
        }
        .data();

        // TODO: unused data to track fee_bps
        let fee_bps: u16 = 0;
        data.extend(fee_bps.to_le_bytes());

        let mut instruction = Instruction {
            program_id: order_engine::ID,
            accounts: order_engine::accounts::Fill {
                maker: *maker,
                taker: *taker,
                taker_input_mint_token_account: *taker_input_mint_token_account,
                maker_input_mint_token_account: *maker_input_mint_token_account,
                taker_output_mint_token_account: *taker_output_mint_token_account,
                maker_output_mint_token_account: *maker_output_mint_token_account,
                input_mint: *input_mint,
                input_token_program: *input_token_program,
                output_mint: *output_mint,
                output_token_program: *output_token_program,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data,
        };
        if let Some(temporary_wsol_token_account) = temporary_wsol_token_account {
            instruction
                .accounts
                .push(AccountMeta::new(*temporary_wsol_token_account, false));
        }

        instruction
    }
}

#[derive(Default, Debug)]
enum AccountKind {
    #[default]
    Token,
    NativeMint,
    NativeSol,
}

#[derive(Default)]
struct Accounts {
    input: AccountKind,
    output: AccountKind,
}

#[derive(Clone, Debug, PartialEq)]
enum ExtensionInitializationParams {
    TransferFeeConfig {
        transfer_fee_config_authority: Option<Pubkey>,
        withdraw_withheld_authority: Option<Pubkey>,
        transfer_fee_basis_points: u16,
        maximum_fee: u64,
    },
    NonTransferable,
}

impl ExtensionInitializationParams {
    fn extension(&self) -> ExtensionType {
        match self {
            Self::TransferFeeConfig { .. } => ExtensionType::TransferFeeConfig,
            Self::NonTransferable => ExtensionType::NonTransferable,
        }
    }

    fn instruction(&self, token_program_id: &Pubkey, mint: &Pubkey) -> Instruction {
        match self {
            Self::TransferFeeConfig {
                transfer_fee_config_authority,
                withdraw_withheld_authority,
                transfer_fee_basis_points,
                maximum_fee,
            } => transfer_fee::instruction::initialize_transfer_fee_config(
                token_program_id,
                mint,
                transfer_fee_config_authority.as_ref(),
                withdraw_withheld_authority.as_ref(),
                *transfer_fee_basis_points,
                *maximum_fee,
            )
            .unwrap(),
            Self::NonTransferable => spl_token_2022::instruction::initialize_non_transferable_mint(
                token_program_id,
                mint,
            )
            .unwrap(),
        }
    }
}

#[derive(Default)]
struct TestMode {
    taker_accounts: Accounts,
    maker_accounts: Accounts,
    expected_error: Option<TransactionError>,
    input_mint_extensions: Option<Vec<ExtensionInitializationParams>>,
    output_mint_extensions: Option<Vec<ExtensionInitializationParams>>,
}

struct TestToken {
    mint: Pubkey,
    program_id: Pubkey,
}

impl TestToken {
    fn new(mint: Pubkey, program_id: Pubkey) -> Self {
        Self { mint, program_id }
    }

    fn get_associated_token_address(&self, owner: &Pubkey) -> Pubkey {
        get_associated_token_address_with_program_id(owner, &self.mint, &self.program_id)
    }
}

const TEST_AIRDROP: u64 = 5 * LAMPORTS_PER_SOL;

fn prepare_test(test_mode: TestMode) -> TestEnvironment {
    let mut svm = LiteSVM::new();
    ensure_native_mint(&mut svm);
    let program_path = order_engine_program_path();
    svm.add_program_from_file(order_engine::ID, &program_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to load order_engine program from {}: {error}. Build the SBF program first, for example with `cargo build-sbf --manifest-path programs/order-engine/Cargo.toml --sbf-out-dir target/deploy`.",
                program_path.display()
            )
        });

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), TEST_AIRDROP * 10).unwrap();

    let taker_keypair = Keypair::new();
    let taker = taker_keypair.pubkey();

    let maker_keypair = Keypair::new();
    let maker = maker_keypair.pubkey();

    // Fund the taker and the maker.
    process_and_assert_ok(
        &[
            system_instruction::transfer(&payer.pubkey(), &taker, TEST_AIRDROP),
            system_instruction::transfer(&payer.pubkey(), &maker, TEST_AIRDROP),
        ],
        &payer,
        &[&payer],
        &mut svm,
    );

    let (mut mint_a_keypair, mut mint_a, mut mint_b_keypair, mut mint_b) = {
        let mint_a_keypair = Keypair::new();
        let mint_a = mint_a_keypair.pubkey();
        let mint_b_keypair = Keypair::new();
        let mint_b = mint_b_keypair.pubkey();
        (Some(mint_a_keypair), mint_a, Some(mint_b_keypair), mint_b)
    };

    let mut uses_temporary_wsol_token_account = false;

    // Taker order construction, taker wants some token b for some token a.
    // Using a rate of 1 LST => 150 USDC.
    let input_amount = 1_000_000_000;
    let output_amount = 150_000_000;

    let TestMode {
        taker_accounts,
        maker_accounts,
        expected_error: _,
        input_mint_extensions,
        output_mint_extensions,
    } = test_mode;
    match (&taker_accounts, &maker_accounts) {
        (
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::Token,
            },
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::Token,
            },
        ) => (),
        (
            Accounts {
                input: AccountKind::NativeSol,
                output: AccountKind::Token,
            },
            Accounts {
                input: AccountKind::NativeSol,
                output: AccountKind::Token,
            },
        )
        | (
            Accounts {
                input: AccountKind::NativeMint,
                output: AccountKind::Token,
            },
            Accounts {
                input: AccountKind::NativeMint,
                output: AccountKind::Token,
            },
        ) => {
            mint_a_keypair = None;
            mint_a = spl_token::native_mint::ID;
        }
        (
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeSol,
            },
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeSol,
            },
        )
        | (
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeMint,
            },
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeMint,
            },
        ) => {
            mint_b_keypair = None;
            mint_b = spl_token::native_mint::ID;
        }
        (
            Accounts {
                input: AccountKind::NativeMint,
                output: AccountKind::Token,
            },
            Accounts {
                input: AccountKind::NativeSol,
                output: AccountKind::Token,
            },
        )
        | (
            Accounts {
                input: AccountKind::NativeSol,
                output: AccountKind::Token,
            },
            Accounts {
                input: AccountKind::NativeMint,
                output: AccountKind::Token,
            },
        ) => {
            mint_a_keypair = None;
            mint_a = spl_token::native_mint::ID;
            uses_temporary_wsol_token_account = true;
        }
        (
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeMint,
            },
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeSol,
            },
        )
        | (
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeSol,
            },
            Accounts {
                input: AccountKind::Token,
                output: AccountKind::NativeMint,
            },
        ) => {
            mint_b_keypair = None;
            mint_b = spl_token::native_mint::ID;
            uses_temporary_wsol_token_account = true;
        }
        _ => panic!("Invalid combo"),
    };

    // Setup 2 mints.
    let token_a_program_id = if input_mint_extensions.is_some() {
        spl_token_2022::ID
    } else {
        spl_token::ID
    };
    let token_a = TestToken::new(mint_a, token_a_program_id);
    if let Some(mint_a_keypair) = &mint_a_keypair {
        create_mint(
            &mut svm,
            &payer,
            mint_a_keypair,
            token_a_program_id,
            input_mint_extensions.unwrap_or_default(),
        );
    }

    let token_b_program_id = if output_mint_extensions.is_some() {
        spl_token_2022::ID
    } else {
        spl_token::ID
    };
    let token_b = TestToken::new(mint_b, token_b_program_id);
    if let Some(mint_b_keypair) = &mint_b_keypair {
        create_mint(
            &mut svm,
            &payer,
            mint_b_keypair,
            token_b_program_id,
            output_mint_extensions.unwrap_or_default(),
        );
    }

    let amount_with_account_kinds = [Some(input_amount), None, None, Some(output_amount)]
        .into_iter()
        .zip([
            &taker_accounts.input,
            &taker_accounts.output,
            &maker_accounts.input,
            &maker_accounts.output,
        ]);

    let mut taker_input_mint_token_account = None;
    let mut taker_output_mint_token_account = None;
    let mut maker_input_mint_token_account = None;
    let mut maker_output_mint_token_account = None;

    for (((user, token), token_account), amount_with_kind) in [taker, maker]
        .into_iter()
        .cartesian_product([&token_a, &token_b])
        .zip([
            &mut taker_input_mint_token_account,
            &mut taker_output_mint_token_account,
            &mut maker_input_mint_token_account,
            &mut maker_output_mint_token_account,
        ])
        .zip(amount_with_account_kinds)
    {
        println!("{user} {} {:?}", token.mint, amount_with_kind.1);
        let ata = token.get_associated_token_address(&user);
        let set_ata = match amount_with_kind {
            (amount, AccountKind::Token) => {
                create_ata(&mut svm, &payer, &user, token);

                if let Some(amount) = amount {
                    mint_to(&mut svm, &payer, token, &ata, amount);
                }
                true
            }
            (None, AccountKind::NativeMint) => {
                create_ata(&mut svm, &payer, &user, token);
                true
            }
            (Some(amount), AccountKind::NativeMint) => {
                // Send enough.
                process_and_assert_ok(
                    &[system_instruction::transfer(
                        &payer.pubkey(),
                        &ata,
                        amount + 100_000_000,
                    )],
                    &payer,
                    &[&payer],
                    &mut svm,
                );
                create_ata(&mut svm, &payer, &user, token);
                true
            }
            (_, AccountKind::NativeSol) => {
                // Nothing to setup.
                false
            }
        };
        if set_ata {
            *token_account = Some(ata);
        }
    }

    let temporary_wsol_token_account = if uses_temporary_wsol_token_account {
        Some(
            Pubkey::find_program_address(
                &[order_engine::TEMPORARY_WSOL_TOKEN_ACCOUNT, maker.as_ref()],
                &order_engine::ID,
            )
            .0,
        )
    } else {
        None
    };

    TestEnvironment {
        svm,
        payer,
        taker_keypair,
        maker_keypair,
        input_amount,
        output_amount,
        maker,
        taker,
        taker_input_mint_token_account,
        maker_input_mint_token_account,
        taker_output_mint_token_account,
        maker_output_mint_token_account,
        input_mint: token_a.mint,
        input_token_program: token_a_program_id,
        output_mint: token_b.mint,
        output_token_program: token_b_program_id,
        temporary_wsol_token_account,
    }
}

fn ensure_native_mint(svm: &mut LiteSVM) {
    let mut data = vec![0; spl_token::state::Mint::LEN];
    spl_token::state::Mint::pack(
        spl_token::state::Mint {
            decimals: spl_token::native_mint::DECIMALS,
            is_initialized: true,
            ..Default::default()
        },
        &mut data,
    )
    .unwrap();

    svm.set_account(
        spl_token::native_mint::ID,
        Account {
            lamports: svm.minimum_balance_for_rent_exemption(data.len()),
            data,
            owner: spl_token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn order_engine_program_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/deploy/order_engine.so");
    path
}

fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    token_program_id: Pubkey,
    extension_initialization_params: Vec<ExtensionInitializationParams>,
) {
    let extension_types = extension_initialization_params
        .iter()
        .map(ExtensionInitializationParams::extension)
        .collect::<Vec<_>>();
    let space = if token_program_id == spl_token_2022::ID {
        ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&extension_types)
            .unwrap()
    } else {
        spl_token::state::Mint::LEN
    };
    let mut instructions = vec![system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(space),
        space as u64,
        &token_program_id,
    )];

    for params in extension_initialization_params {
        instructions.push(params.instruction(&token_program_id, &mint.pubkey()));
    }

    let initialize_mint_instruction = if token_program_id == spl_token_2022::ID {
        spl_token_2022::instruction::initialize_mint(
            &token_program_id,
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            9,
        )
        .unwrap()
    } else {
        spl_token::instruction::initialize_mint(
            &token_program_id,
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            9,
        )
        .unwrap()
    };
    instructions.push(initialize_mint_instruction);

    process_and_assert_ok(&instructions, payer, &[payer, mint], svm);
}

fn create_ata(svm: &mut LiteSVM, payer: &Keypair, owner: &Pubkey, token: &TestToken) {
    process_and_assert_ok(
        &[create_associated_token_account(
            &payer.pubkey(),
            owner,
            &token.mint,
            &token.program_id,
        )],
        payer,
        &[payer],
        svm,
    );
}

fn mint_to(
    svm: &mut LiteSVM,
    payer: &Keypair,
    token: &TestToken,
    token_account: &Pubkey,
    amount: u64,
) {
    let instruction = if token.program_id == spl_token_2022::ID {
        spl_token_2022::instruction::mint_to(
            &token.program_id,
            &token.mint,
            token_account,
            &payer.pubkey(),
            &[],
            amount,
        )
        .unwrap()
    } else {
        spl_token::instruction::mint_to(
            &token.program_id,
            &token.mint,
            token_account,
            &payer.pubkey(),
            &[],
            amount,
        )
        .unwrap()
    };
    process_and_assert_ok(&[instruction], payer, &[payer], svm);
}

pub fn process_and_assert_ok(
    instructions: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
    svm: &mut LiteSVM,
) {
    let result = process_instructions(instructions, payer, signers, svm);
    assert_matches!(result, Ok(_));
}

pub fn process_instructions(
    instructions: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
    svm: &mut LiteSVM,
) -> std::result::Result<litesvm::types::TransactionMetadata, FailedTransactionMetadata> {
    let mut all_signers = vec![payer];
    all_signers.extend_from_slice(signers);

    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &all_signers,
        svm.latest_blockhash(),
    );

    println!("TX size: {}", bincode::serialize(&tx).unwrap().len());

    svm.send_transaction(tx)
}

fn token_account_amount(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let account = svm.get_account(account).unwrap();
    if account.owner == spl_token_2022::ID {
        StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account.data)
            .unwrap()
            .base
            .amount
    } else {
        spl_token::state::Account::unpack(&account.data)
            .unwrap()
            .amount
    }
}
