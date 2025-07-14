use anchor_lang::prelude::*;

const PUMP_MINT: Pubkey = pubkey!("pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn");
const DLMM_TEST_MINT: Pubkey = pubkey!("8wj53J4y684unTbmR9KmoVLnLmT8Pk73uToDauWYzdWD");

pub const WHITELISTED_TRANSFER_HOOK_MINTS: [Pubkey; 2] = [PUMP_MINT, DLMM_TEST_MINT];
