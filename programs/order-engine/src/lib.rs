use anchor_lang::prelude::*;

pub mod error;
mod instructions;

use instructions::*;

#[constant]
pub const TEMPORARY_WSOL_TOKEN_ACCOUNT: &[u8] = instructions::TEMPORARY_WSOL_TOKEN_ACCOUNT;

#[cfg(not(feature = "production"))]
declare_id!("RderEngine111111111111111111111111111111112");

#[cfg(feature = "production")]
declare_id!("61DFfeTKM7trxYcPQCM78bJ794ddZprZpAwAnLiwTpYH");

#[program]
pub mod order_engine {
    use super::*;

    pub fn fill<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, Fill<'info>>,
        input_amount: u64,
        output_amount: u64,
        expire_at: i64,
        fee_bps: u16,
    ) -> Result<()> {
        handle_fill(ctx, input_amount, output_amount, expire_at, fee_bps)
    }
}
