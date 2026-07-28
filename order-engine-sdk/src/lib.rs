use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::BorrowedAccountMeta;

declare_program!(order_engine);

pub mod fill;
pub mod transaction;
pub mod parse_util;

pub fn account_pubkeys(accounts: &[BorrowedAccountMeta]) -> Vec<Pubkey> {
    accounts.iter().map(|meta| *meta.pubkey).collect()
}

