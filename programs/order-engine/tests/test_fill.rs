use std::sync::Arc;

use anchor_lang::{
    prelude::*,
    solana_program::{self, instruction::Instruction},
    system_program, InstructionData,
};
use anchor_spl::token::spl_token::native_mint;
use assert_matches::assert_matches;
use itertools::Itertools;
use solana_program_test::{
    tokio::{self, sync::Mutex},
    BanksClient, BanksClientError, ProgramTest,
};
use solana_sdk::{
    feature_set::bpf_account_data_direct_mapping, native_token::LAMPORTS_PER_SOL,
    signature::Keypair, signer::Signer, system_instruction, transaction::Transaction,
    transaction::TransactionError,
};
use spl_token_client::{
    client::{
        ProgramBanksClient, ProgramBanksClientProcessTransaction, SendTransaction,
        SimulateTransaction,
    },
    token::{ExtensionInitializationParams, Token},
};
use test_case::test_case;

async fn get_amount_or_lamports(
    token: &Token<ProgramBanksClientProcessTransaction>,
    user: Pubkey,
    token_account: &Option<Pubkey>,
) -> u64 {
    match token_account {
        Some(token_account) => token.get_amount(token_account).await,
        None => token.get_account(user).await.unwrap().lamports,
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
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, input_mint_extensions: Some(vec![ExtensionInitializationParams::TransferFeeConfig { transfer_fee_config_authority: None, withdraw_withheld_authority: None, transfer_fee_basis_points: 100, maximum_fee: u64::MAX }]), expected_error: Some(TransactionError::InstructionError(0, solana_sdk::instruction::InstructionError::Custom(u32::from(order_engine::error::OrderEngineError::Token2022MintExtensionNotSupported)))), ..Default::default()})]
#[test_case(TestMode { taker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, maker_accounts: Accounts { input: AccountKind::Token, output: AccountKind::Token }, input_mint_extensions: Some(vec![ExtensionInitializationParams::NonTransferable]), expected_error: Some(TransactionError::InstructionError(0, solana_sdk::instruction::InstructionError::Custom(anchor_spl::token_2022::spl_token_2022::error::TokenError::NonTransferable as u32))), ..Default::default()})]
#[tokio::test]
async fn test_fill(test_mode: TestMode) {
    let expected_error = test_mode.expected_error.clone();
    let test_environment = prepare_test(test_mode).await;

    let fill_instruction = test_environment.create_fill_instruction();
    let TestEnvironment {
        banks_client,
        payer,
        taker_keypair,
        maker_keypair,
        input_amount,
        output_amount,
        maker,
        taker,
        // TODO: Maker assertions
        taker_input_mint_token_account,
        maker_input_mint_token_account,
        taker_output_mint_token_account,
        maker_output_mint_token_account,
        input_token,
        output_token,
        ..
    } = test_environment;

    let taker_balance_reader = BalanceReader::new(&input_token, taker, &None);
    let taker_input_balance_reader =
        BalanceReader::new(&input_token, taker, &taker_input_mint_token_account);
    let taker_output_balance_reader =
        BalanceReader::new(&output_token, taker, &taker_output_mint_token_account);

    let before_taker_balance = taker_balance_reader.get_balance().await;
    let before_taker_input_amount = taker_input_balance_reader.get_balance().await;
    let before_taker_output_amount = taker_output_balance_reader.get_balance().await;

    let maker_balance_reader = BalanceReader::new(&input_token, taker, &None);
    let maker_input_balance_reader =
        BalanceReader::new(&input_token, maker, &maker_input_mint_token_account);
    let maker_output_balance_reader =
        BalanceReader::new(&output_token, maker, &maker_output_mint_token_account);

    let before_maker_balance = maker_balance_reader.get_balance().await;
    let before_maker_input_amount = maker_input_balance_reader.get_balance().await;
    let before_maker_output_amount = maker_output_balance_reader.get_balance().await;

    // Maker fills
    let result = process_instructions(
        &[fill_instruction],
        &payer, // To simplify accounting we make the payer another keypair, this has no impact on the fill instruction
        &[&taker_keypair, &maker_keypair],
        &banks_client,
    )
    .await;

    match expected_error {
        Some(expected_error) => {
            let BanksClientError::TransactionError(transaction_error) = result.unwrap_err() else {
                panic!("The error was not a transaction error");
            };
            assert_eq!(transaction_error, expected_error);
            return;
        }
        None => {
            assert_matches!(result, Ok(()));
        }
    }

    let after_taker_balance = taker_balance_reader.get_balance().await;
    let after_taker_input_amount = taker_input_balance_reader.get_balance().await;
    let after_taker_output_amount = taker_output_balance_reader.get_balance().await;

    let after_maker_balance = maker_balance_reader.get_balance().await;
    let after_maker_input_amount = maker_input_balance_reader.get_balance().await;
    let after_maker_output_amount = maker_output_balance_reader.get_balance().await;

    println!("{before_taker_input_amount} {after_taker_input_amount}");
    assert_eq!(
        before_taker_input_amount.checked_sub(after_taker_input_amount),
        Some(input_amount)
    );
    println!("{after_taker_output_amount:?} {before_taker_output_amount:?}");
    assert_eq!(
        after_taker_output_amount.checked_sub(before_taker_output_amount),
        Some(output_amount)
    );

    // The native sol balance is not already checked so assert no change
    if taker_input_mint_token_account.is_none() && taker_output_mint_token_account.is_none() {
        assert_eq!(before_taker_balance, after_taker_balance);
    }

    println!("{after_maker_input_amount} {before_maker_input_amount}");
    assert_eq!(
        after_maker_input_amount.checked_sub(before_maker_input_amount),
        Some(input_amount)
    );
    println!("{before_maker_output_amount:?} {after_maker_output_amount:?}");
    assert_eq!(
        before_maker_output_amount.checked_sub(after_maker_output_amount),
        Some(output_amount)
    );

    // The native sol balance is not already checked so assert no change
    if maker_input_mint_token_account.is_none() && maker_output_mint_token_account.is_none() {
        assert_eq!(before_maker_balance, after_maker_balance);
    }
}

struct BalanceReader<'a> {
    token: &'a Token<ProgramBanksClientProcessTransaction>,
    user: Pubkey,
    token_account: &'a Option<Pubkey>,
}

impl<'a> BalanceReader<'a> {
    fn new(
        token: &'a Token<ProgramBanksClientProcessTransaction>,
        user: Pubkey,
        token_account: &'a Option<Pubkey>,
    ) -> Self {
        Self {
            token,
            user,
            token_account,
        }
    }

    async fn get_balance(&self) -> u64 {
        get_amount_or_lamports(self.token, self.user, self.token_account).await
    }
}

struct TestEnvironment {
    banks_client: Arc<Mutex<BanksClient>>,
    payer: Arc<Keypair>,
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
    input_token: Token<ProgramBanksClientProcessTransaction>,
    output_token: Token<ProgramBanksClientProcessTransaction>,
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

#[derive(Default)]
struct TestMode {
    taker_accounts: Accounts,
    maker_accounts: Accounts,
    expected_error: Option<TransactionError>,
    input_mint_extensions: Option<Vec<ExtensionInitializationParams>>,
    output_mint_extensions: Option<Vec<ExtensionInitializationParams>>,
}

const TEST_AIRDROP: u64 = 5 * LAMPORTS_PER_SOL;

async fn prepare_test(test_mode: TestMode) -> TestEnvironment {
    let mut pt = ProgramTest::new(
        "order_engine",
        order_engine::ID,
        anchor_processor!(order_engine),
    );
    pt.deactivate_feature(bpf_account_data_direct_mapping::ID);

    let (banks_client, payer, _) = pt.start().await;

    let taker_keypair = Keypair::new();
    let taker = taker_keypair.pubkey();

    let maker_keypair = Keypair::new();
    let maker = maker_keypair.pubkey();

    let payer = Arc::new(payer);

    let banks_client = Arc::new(Mutex::new(banks_client));
    let client = Arc::new(ProgramBanksClient::new_from_client(
        banks_client.clone(),
        ProgramBanksClientProcessTransaction,
    ));

    // Fund the taker and the maker
    process_and_assert_ok(
        &[
            system_instruction::transfer(&payer.pubkey(), &taker, TEST_AIRDROP),
            system_instruction::transfer(&payer.pubkey(), &maker, TEST_AIRDROP),
        ],
        &payer,
        &[&payer],
        &banks_client,
    )
    .await;

    let (mut mint_a_keypair, mut mint_a, mut mint_b_keypair, mut mint_b) = {
        let mint_a_keypair = Keypair::new();
        let mint_a = mint_a_keypair.pubkey();
        let mint_b_keypair = Keypair::new();
        let mint_b = mint_b_keypair.pubkey();
        (Some(mint_a_keypair), mint_a, Some(mint_b_keypair), mint_b)
    };

    let mut uses_temporary_wsol_token_account = false;

    // Taker order construction, taker wants some token b for some token a
    // Using a rate of 1 LST => 150 USDC
    let input_amount = 1_000_000_000;
    let output_amount = 150_000_000;

    let TestMode {
        taker_accounts,
        maker_accounts,
        expected_error,
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
            mint_a = native_mint::ID;
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
            mint_b = native_mint::ID;
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
            mint_a = native_mint::ID;
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
            mint_b = native_mint::ID;
            uses_temporary_wsol_token_account = true;
        }
        _ => panic!("Invalid combo"),
    };

    // Setup 2 mints
    let token_a_program_id = if input_mint_extensions.is_some() {
        anchor_spl::token_2022::ID
    } else {
        anchor_spl::token::ID
    };
    let token_a = Token::new(
        client.clone(),
        &token_a_program_id,
        &mint_a,
        Some(9),
        payer.clone(),
    );
    if let Some(mint_a_keypair) = &mint_a_keypair {
        token_a
            .create_mint(
                &payer.pubkey(),
                None,
                input_mint_extensions.unwrap_or_default(),
                &[mint_a_keypair],
            )
            .await
            .unwrap();
    }

    let token_b_program_id = if output_mint_extensions.is_some() {
        anchor_spl::token_2022::ID
    } else {
        anchor_spl::token::ID
    };
    let token_b = Token::new(
        client.clone(),
        &token_b_program_id,
        &mint_b,
        Some(9),
        payer.clone(),
    );
    if let Some(mint_b_keypair) = &mint_b_keypair {
        token_b
            .create_mint(
                &payer.pubkey(),
                None,
                output_mint_extensions.unwrap_or_default(),
                &[mint_b_keypair],
            )
            .await
            .unwrap();
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
        println!("{user} {} {:?}", token.get_address(), amount_with_kind.1);
        let ata = token.get_associated_token_address(&user);
        let set_ata = match amount_with_kind {
            (amount, AccountKind::Token) => {
                token.create_associated_token_account(&user).await.unwrap();

                if let Some(amount) = amount {
                    token
                        .mint_to(&ata, &payer.pubkey(), amount, &[&payer])
                        .await
                        .unwrap();
                }
                true
            }
            (None, AccountKind::NativeMint) => {
                token.create_associated_token_account(&user).await.unwrap();
                true
            }
            (Some(amount), AccountKind::NativeMint) => {
                // Send enough
                process_and_assert_ok(
                    &[system_instruction::transfer(
                        &payer.pubkey(),
                        &ata,
                        amount + 100_000_000,
                    )],
                    &payer,
                    &[&payer],
                    &banks_client,
                )
                .await;
                token.create_associated_token_account(&user).await.unwrap();
                true
            }
            (_, AccountKind::NativeSol) => {
                // Nothing to setup
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
        banks_client,
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
        input_mint: *token_a.get_address(),
        input_token_program: token_a_program_id,
        output_mint: *token_b.get_address(),
        output_token_program: token_b_program_id,
        input_token: token_a,
        output_token: token_b,
        temporary_wsol_token_account,
    }
}

pub async fn process_and_assert_ok(
    instructions: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
    banks_client: &Mutex<BanksClient>,
) {
    let result = process_instructions(instructions, payer, signers, banks_client).await;
    assert_matches!(result, Ok(()));
}
pub async fn process_instructions(
    instructions: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
    banks_client: &Mutex<BanksClient>,
) -> std::result::Result<(), BanksClientError> {
    let mut banks_client = banks_client.lock().await;
    let recent_blockhash = banks_client.get_latest_blockhash().await.unwrap();

    let mut all_signers = vec![payer];
    all_signers.extend_from_slice(signers);

    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &all_signers,
        recent_blockhash,
    );

    println!("TX size: {}", bincode::serialize(&tx).unwrap().len());

    banks_client.process_transaction(tx).await
}

/// Workaround from anchor issue https://github.com/coral-xyz/anchor/issues/2738#issuecomment-2230683481
#[macro_export]
macro_rules! anchor_processor {
    ($program:ident) => {{
        fn entry(
            program_id: &solana_program::pubkey::Pubkey,
            accounts: &[solana_program::account_info::AccountInfo],
            instruction_data: &[u8],
        ) -> solana_program::entrypoint::ProgramResult {
            let accounts = Box::leak(Box::new(accounts.to_vec()));

            $program::entry(program_id, accounts, instruction_data)
        }

        solana_program_test::processor!(entry)
    }};
}

trait TokenExtra {
    async fn get_amount(&self, account: &Pubkey) -> u64;
}

impl<T> TokenExtra for Token<T>
where
    T: SendTransaction + SimulateTransaction,
{
    async fn get_amount(&self, account: &Pubkey) -> u64 {
        self.get_account_info(account).await.unwrap().base.amount
    }
}
