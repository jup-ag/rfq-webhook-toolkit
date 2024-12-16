use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, Debug)]

pub struct TakerVault {
    pub bump: u8,
    pub taker: Pubkey,
    pub nonce: u64,
    pub in_flash_fill: bool,
}
