use anchor_lang::{prelude::*, solana_program::program_pack::Pack, system_program};
use anchor_spl::{
    associated_token::spl_associated_token_account::tools::account::create_pda_account,
    token::{
        self,
        spl_token::{self, native_mint},
    },
    token_interface::{TokenAccount, TokenInterface},
};

use crate::error::OrderEngineError;

pub const TEMPORARY_WSOL_TOKEN_ACCOUNT: &[u8] = b"temporary-wsol-token-account";

pub fn handle_fill<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Fill<'info>>,
    input_amount: u64,
    output_amount: u64,
    expire_at: i64,
) -> Result<()> {
    require_gte!(expire_at, Clock::get()?.unix_timestamp);

    match (
        &ctx.accounts.taker_input_mint_token_account,
        &ctx.accounts.maker_input_mint_token_account,
    ) {
        (None, None) => {
            require_keys_eq!(ctx.accounts.input_mint.key(), native_mint::ID);

            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.taker.to_account_info(),
                        to: ctx.accounts.maker.to_account_info(),
                    },
                ),
                input_amount,
            )?;
        }
        (None, Some(maker_input_mint_token_account)) => {
            require_keys_eq!(ctx.accounts.input_mint.key(), native_mint::ID);

            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.taker.to_account_info(),
                        to: maker_input_mint_token_account.to_account_info(),
                    },
                ),
                input_amount,
            )?;
            token::sync_native(CpiContext::new(
                ctx.accounts.input_token_program.to_account_info(),
                token::SyncNative {
                    account: maker_input_mint_token_account.to_account_info(),
                },
            ))?;
        }
        (Some(taker_input_mint_token_account), None) => {
            require_keys_eq!(ctx.accounts.input_mint.key(), native_mint::ID);

            unwrap_sol(
                ctx.accounts.maker.to_account_info(),
                ctx.accounts.taker.to_account_info(),
                taker_input_mint_token_account.to_account_info(),
                None,
                ctx.remaining_accounts.iter().next(),
                ctx.accounts.input_mint.to_account_info(),
                ctx.accounts.input_token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                input_amount,
            )?;
        }
        (Some(taker_input_mint_token_account), Some(maker_input_mint_token_account)) => {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.input_token_program.to_account_info(),
                    token::Transfer {
                        from: taker_input_mint_token_account.to_account_info(),
                        to: maker_input_mint_token_account.to_account_info(),
                        authority: ctx.accounts.taker.to_account_info(),
                    },
                ),
                input_amount,
            )?;
        }
    }

    match (
        &ctx.accounts.maker_output_mint_token_account,
        &ctx.accounts.taker_output_mint_token_account,
    ) {
        (None, None) => {
            require_keys_eq!(ctx.accounts.output_mint.key(), native_mint::ID);

            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.maker.to_account_info(),
                        to: ctx.accounts.taker.to_account_info(),
                    },
                ),
                output_amount,
            )?;
        }
        (Some(maker_output_mint_token_account), None) => {
            require_keys_eq!(ctx.accounts.output_mint.key(), native_mint::ID);

            unwrap_sol(
                ctx.accounts.maker.to_account_info(),
                ctx.accounts.maker.to_account_info(),
                maker_output_mint_token_account.to_account_info(),
                Some(ctx.accounts.taker.to_account_info()),
                ctx.remaining_accounts.iter().next(),
                ctx.accounts.output_mint.to_account_info(),
                ctx.accounts.output_token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                output_amount,
            )?;
        }
        (None, Some(taker_output_mint_token_account)) => {
            require_keys_eq!(ctx.accounts.output_mint.key(), native_mint::ID);

            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.maker.to_account_info(),
                        to: taker_output_mint_token_account.to_account_info(),
                    },
                ),
                output_amount,
            )?;
            token::sync_native(CpiContext::new(
                ctx.accounts.output_token_program.to_account_info(),
                token::SyncNative {
                    account: taker_output_mint_token_account.to_account_info(),
                },
            ))?;
        }
        (Some(maker_output_mint_token_account), Some(taker_output_mint_token_account)) => {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.output_token_program.to_account_info(),
                    token::Transfer {
                        from: maker_output_mint_token_account.to_account_info(),
                        to: taker_output_mint_token_account.to_account_info(),
                        authority: ctx.accounts.maker.to_account_info(),
                    },
                ),
                output_amount,
            )?;
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct Fill<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(
        mut,
        token::authority = taker,
        token::mint = input_mint,
        token::token_program = input_token_program
    )]
    pub taker_input_mint_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    #[account(
        mut,
        token::authority = maker,
        token::mint = input_mint,
        token::token_program = input_token_program
    )]
    pub maker_input_mint_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    #[account(
        mut,
        token::authority = taker,
        token::mint = output_mint,
        token::token_program = output_token_program
    )]
    pub taker_output_mint_token_account: Option<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::authority = maker,
        token::mint = output_mint,
        token::token_program = output_token_program
    )]
    pub maker_output_mint_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    /// CHECK: Validated by token account mint check
    pub input_mint: UncheckedAccount<'info>,
    pub input_token_program: Interface<'info, TokenInterface>,
    /// CHECK: Validated by token account mint check
    pub output_mint: UncheckedAccount<'info>,
    pub output_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

fn unwrap_sol<'info>(
    maker: AccountInfo<'info>,
    sender: AccountInfo<'info>,
    sender_token_account: AccountInfo<'info>,
    receiver: Option<AccountInfo<'info>>,
    temporary_wsol_token_account: Option<&AccountInfo<'info>>,
    wsol_mint: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let temporary_wsol_token_account = temporary_wsol_token_account
        .ok_or(OrderEngineError::MissingTemporaryWrappedSolTokenAccount)?;

    let (expected_temporary_wsol_token_account, bump) = Pubkey::find_program_address(
        &[TEMPORARY_WSOL_TOKEN_ACCOUNT, maker.key.as_ref()],
        &crate::ID,
    );
    require_keys_eq!(
        temporary_wsol_token_account.key(),
        expected_temporary_wsol_token_account
    );
    let new_pda_signer_seeds: &[&[u8]] =
        &[TEMPORARY_WSOL_TOKEN_ACCOUNT, maker.key.as_ref(), &[bump]];
    create_pda_account(
        &maker,
        &Rent::get()?,
        spl_token::state::Account::LEN,
        &spl_token::ID,
        &system_program,
        temporary_wsol_token_account,
        new_pda_signer_seeds,
    )?;
    token::initialize_account3(CpiContext::new(
        token_program.to_account_info(),
        token::InitializeAccount3 {
            account: temporary_wsol_token_account.clone(),
            mint: wsol_mint,
            authority: maker.clone(),
        },
    ))?;

    token::transfer(
        CpiContext::new(
            token_program.clone(),
            token::Transfer {
                from: sender_token_account.clone(),
                to: temporary_wsol_token_account.clone(),
                authority: sender.clone(),
            },
        ),
        amount,
    )?;

    // Close temporary wsol token account into the maker
    token::close_account(CpiContext::new(
        token_program.to_account_info(),
        token::CloseAccount {
            account: temporary_wsol_token_account.clone(),
            destination: maker.clone(),
            authority: maker.clone(),
        },
    ))?;

    if let Some(receiver) = receiver {
        // Transfer native sol to receipient
        system_program::transfer(
            CpiContext::new(
                system_program,
                system_program::Transfer {
                    from: maker,
                    to: receiver,
                },
            ),
            amount,
        )?;
    }

    Ok(())
}
