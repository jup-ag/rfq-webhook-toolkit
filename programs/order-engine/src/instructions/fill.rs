use anchor_lang::{prelude::*, solana_program::program::invoke, system_program};
use anchor_spl::{
    token::{self},
    token_interface::{self, spl_pod::primitives::PodU16, TokenAccount, TokenInterface},
};
use spl_token_2022_interface::{
    self as spl_token_2022,
    extension::{transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions},
};
use spl_token_interface::{self as spl_token, native_mint};

use crate::error::OrderEngineError;

pub fn handle_fill<'info>(
    ctx: Context<'info, Fill<'info>>,
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
                    ctx.accounts.system_program.key(),
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
                    ctx.accounts.system_program.key(),
                    system_program::Transfer {
                        from: ctx.accounts.taker.to_account_info(),
                        to: maker_input_mint_token_account.to_account_info(),
                    },
                ),
                input_amount,
            )?;
            token::sync_native(CpiContext::new(
                ctx.accounts.input_token_program.key(),
                token::SyncNative {
                    account: maker_input_mint_token_account.to_account_info(),
                },
            ))?;
        }
        (Some(taker_input_mint_token_account), None) => {
            require_keys_eq!(ctx.accounts.input_mint.key(), native_mint::ID);

            unwrap_sol(
                ctx.accounts.taker.to_account_info(),
                taker_input_mint_token_account.to_account_info(),
                ctx.accounts.maker.to_account_info(),
                ctx.accounts.input_token_program.to_account_info(),
                input_amount,
            )?;
        }
        (Some(taker_input_mint_token_account), Some(maker_input_mint_token_account)) => transfer(
            ctx.accounts.input_token_program.to_account_info(),
            taker_input_mint_token_account.to_account_info(),
            maker_input_mint_token_account.to_account_info(),
            ctx.accounts.taker.to_account_info(),
            ctx.accounts.input_mint.to_account_info(),
            input_amount,
        )?,
    }

    match (
        &ctx.accounts.maker_output_mint_token_account,
        &ctx.accounts.taker_output_mint_token_account,
    ) {
        (None, None) => {
            require_keys_eq!(ctx.accounts.output_mint.key(), native_mint::ID);

            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.key(),
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
                maker_output_mint_token_account.to_account_info(),
                ctx.accounts.taker.to_account_info(),
                ctx.accounts.output_token_program.to_account_info(),
                output_amount,
            )?;
        }
        (None, Some(taker_output_mint_token_account)) => {
            require_keys_eq!(ctx.accounts.output_mint.key(), native_mint::ID);

            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.key(),
                    system_program::Transfer {
                        from: ctx.accounts.maker.to_account_info(),
                        to: taker_output_mint_token_account.to_account_info(),
                    },
                ),
                output_amount,
            )?;
            token::sync_native(CpiContext::new(
                ctx.accounts.output_token_program.key(),
                token::SyncNative {
                    account: taker_output_mint_token_account.to_account_info(),
                },
            ))?;
        }
        (Some(maker_output_mint_token_account), Some(taker_output_mint_token_account)) => transfer(
            ctx.accounts.output_token_program.to_account_info(),
            maker_output_mint_token_account.to_account_info(),
            taker_output_mint_token_account.to_account_info(),
            ctx.accounts.maker.to_account_info(),
            ctx.accounts.output_mint.to_account_info(),
            output_amount,
        )?,
    }

    Ok(())
}

fn transfer<'info>(
    token_program: AccountInfo<'info>,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let decimals_for_transfer_checked = if token_program.key.eq(&spl_token_2022::ID) {
        let mint_data = mint.try_borrow_data()?;
        let mint_state_with_extensions =
            StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

        if let Ok(transfer_fee_config) =
            mint_state_with_extensions.get_extension::<TransferFeeConfig>()
        {
            require!(
                transfer_fee_config
                    .get_epoch_fee(Clock::get()?.epoch)
                    .transfer_fee_basis_points
                    == PodU16([0; 2]),
                OrderEngineError::Token2022MintExtensionNotSupported
            );
        }

        Some(mint_state_with_extensions.base.decimals)
    } else {
        None
    };

    match decimals_for_transfer_checked {
        Some(decimals) => token_interface::transfer_checked(
            CpiContext::new(
                *token_program.key,
                token_interface::TransferChecked {
                    from,
                    mint,
                    to,
                    authority,
                },
            ),
            amount,
            decimals,
        ),
        None => token::transfer(
            CpiContext::new(
                *token_program.key,
                token::Transfer {
                    from,
                    to,
                    authority,
                },
            ),
            amount,
        ),
    }
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
    authority: AccountInfo<'info>,
    sender_token_account: AccountInfo<'info>,
    receiver: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let instruction = if token_program.key.eq(&spl_token_2022::ID) {
        spl_token_2022::instruction::unwrap_lamports(
            token_program.key,
            sender_token_account.key,
            receiver.key,
            authority.key,
            &[],
            Some(amount),
        )?
    } else {
        spl_token::instruction::unwrap_lamports(
            token_program.key,
            sender_token_account.key,
            receiver.key,
            authority.key,
            &[],
            Some(amount),
        )?
    };

    invoke(&instruction, &[sender_token_account, receiver, authority])?;

    Ok(())
}
