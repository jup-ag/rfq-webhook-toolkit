//! Flash-loan fills: an order-engine fill wrapped in a Jupiter Lend flash loan and an Ondo GM
//! mint or redeem, so the maker needs no inventory. The RFQ builds it with [`wrap`]; the maker
//! checks it with [`crate::fill::validate_flash_fill_sanitized_message`].
//!
//! Mint side (taker buys GM):  `[cu_price, cu_limit, borrow, secp, mint_with_usdc, ...fill..., payback]`
//! Redeem side (taker sells GM): `[cu_price, cu_limit, borrow, ...fill..., secp, redeem_for_usdc, payback]`
//!
//! Account lists and the digest layout were pinned against mainnet transactions of both programs
//! (see the tests). Program sources: `jup-ag/jupiter-lend` (IDLs) and
//! `ondoprotocol/global-markets-solana`.

use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::prelude::*;
use solana_sdk::{
    keccak, pubkey,
    pubkey::Pubkey,
    sysvar::instructions::{BorrowedAccountMeta, BorrowedInstruction},
};
pub use webhook_api::enums::FlashSide;

pub const FLASHLOAN_PROGRAM: Pubkey = pubkey!("jupgfSgfuAXv4B6R2Uxu85Z1qdzgju79s6MfZekN6XS");
pub const LIQUIDITY_PROGRAM: Pubkey = pubkey!("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC");
pub const ONDO_GM_PROGRAM: Pubkey = pubkey!("XzTT4XB8m7sLD2xi6snefSasaswsKCxx5Tifjondogm");
pub const SECP256K1_PROGRAM: Pubkey = pubkey!("KeccakSecp256k11111111111111111111111111111");
pub const SYSVAR_INSTRUCTIONS: Pubkey = pubkey!("Sysvar1nstructions1111111111111111111111111");
pub const SYSTEM_PROGRAM: Pubkey = pubkey!("11111111111111111111111111111111");
pub const TOKEN_PROGRAM: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const TOKEN_2022_PROGRAM: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const ASSOCIATED_TOKEN_PROGRAM: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const USDON_MINT: Pubkey = pubkey!("ZPFtoCe7WWqG4N3ZFRccS8T9SMBeHsd1Vmgv2i7ondo");
/// Pyth USDC/USD price update account the Ondo program is configured with (`usdon_manager_state.usdc_price_update`).
pub const PYTH_USDC_PRICE_UPDATE: Pubkey = pubkey!("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX");
/// Solana mainnet genesis hash; the Ondo program hashes it as the chain id.
pub const MAINNET_CHAIN_ID: Pubkey = pubkey!("5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d");
/// Ethereum address of Ondo's attestation signer on mainnet (`gmtoken_manager_state.attestation_signer_secp`).
pub const ONDO_ATTESTATION_SIGNER: [u8; 20] = [
    0xff, 0x2d, 0x04, 0xf6, 0x00, 0x2c, 0x10, 0xeb, 0xa7, 0x02, 0xad, 0xaf, 0xe2, 0xf4, 0x3c, 0xac,
    0x59, 0x96, 0xc4, 0x77,
];

const FLASHLOAN_BORROW_DISC: [u8; 8] = [103, 19, 78, 24, 240, 9, 135, 63];
const FLASHLOAN_PAYBACK_DISC: [u8; 8] = [213, 47, 153, 137, 84, 243, 94, 232];
const MINT_WITH_USDC_DISC: [u8; 8] = [128, 28, 133, 173, 71, 142, 185, 206];
const REDEEM_FOR_USDC_DISC: [u8; 8] = [150, 9, 215, 220, 255, 157, 74, 78];
const SIDE_BUY: u8 = 0x30;
const SIDE_SELL: u8 = 0x31;

/// A parsed Ondo attestation, ready to be put on-chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    pub id: [u8; 16],
    pub side: FlashSide,
    /// USD per GM token scaled by 1e9
    pub price: u64,
    /// GM base units (9 decimals)
    pub amount: u64,
    pub expiration: i64,
    pub signature: [u8; 64],
    pub recovery_id: u8,
}

impl TryFrom<&webhook_api::responses::FlashFill> for Attestation {
    type Error = anyhow::Error;

    fn try_from(f: &webhook_api::responses::FlashFill) -> Result<Self> {
        let id = f
            .attestation_id
            .parse::<u128>()
            .context("attestationId is not a decimal u128")?
            .to_be_bytes();
        let sig = BASE64_STANDARD
            .decode(&f.signature)
            .context("signature is not base64")?;
        ensure!(
            sig.len() == 65,
            "signature must be 65 bytes, got {}",
            sig.len()
        );
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&sig[..64]);
        // Ethereum-style v (27/28) or a raw recovery id (0/1); the precompile wants 0/1.
        let recovery_id = match sig[64] {
            v @ (0 | 1) => v,
            v @ (27 | 28) => v - 27,
            v => bail!("unexpected signature recovery byte {v}"),
        };
        Ok(Self {
            id,
            side: f.side,
            price: f.price.parse().context("price")?,
            amount: f.amount.parse().context("amount")?,
            expiration: f.expiration,
            signature,
            recovery_id,
        })
    }
}

/// Everything the builder and the validator agree on for one flash fill. The USDC borrowed and
/// repaid is not in here: it is the fill's USDC leg, see [`usdc_leg`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashFillExpectation {
    pub attestation: Attestation,
    pub attestation_signer: [u8; 20],
    pub chain_id: Pubkey,
}

impl FlashFillExpectation {
    pub fn mainnet(attestation: Attestation) -> Self {
        Self {
            attestation,
            attestation_signer: ONDO_ATTESTATION_SIGNER,
            chain_id: MAINNET_CHAIN_ID,
        }
    }
}

/// The USDC flash-borrowed and repaid: what the taker pays when minting, what the taker receives
/// when redeeming. Payback equals the borrow because the flashloan fee is zero; see
/// [`flashloan_fee_bps`].
pub fn usdc_leg(side: FlashSide, in_amount: u64, out_amount: u64) -> u64 {
    match side {
        FlashSide::Mint => in_amount,
        FlashSide::Redeem => out_amount,
    }
}

/// The flashloan fee in bps from the `flashloan_admin` account (discriminator 8, authority 32,
/// liquidity program 32, status 1, fee u16). Flash fills assume 0 and must not be built or signed
/// otherwise: the program's rounding of a nonzero fee is not pinned here.
pub fn flashloan_fee_bps(admin_account_data: &[u8]) -> Result<u16> {
    let bytes = admin_account_data
        .get(73..75)
        .context("flashloan admin account too short")?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

// ---- PDAs -------------------------------------------------------------------------------------

fn pda(seeds: &[&[u8]], program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program).0
}

pub fn flashloan_admin() -> Pubkey {
    pda(&[b"flashloan_admin"], &FLASHLOAN_PROGRAM)
}
pub fn liquidity() -> Pubkey {
    pda(&[b"liquidity"], &LIQUIDITY_PROGRAM)
}
pub fn token_reserve(mint: &Pubkey) -> Pubkey {
    pda(&[b"reserve", mint.as_ref()], &LIQUIDITY_PROGRAM)
}
pub fn rate_model(mint: &Pubkey) -> Pubkey {
    pda(&[b"rate_model", mint.as_ref()], &LIQUIDITY_PROGRAM)
}
pub fn flashloan_borrow_position(mint: &Pubkey) -> Pubkey {
    pda(
        &[
            b"user_borrow_position",
            mint.as_ref(),
            flashloan_admin().as_ref(),
        ],
        &LIQUIDITY_PROGRAM,
    )
}
pub fn liquidity_vault(mint: &Pubkey) -> Pubkey {
    get_associated_token_address_with_program_id(&liquidity(), mint, &TOKEN_PROGRAM)
}

pub fn gm_mint_authority() -> Pubkey {
    pda(&[b"mint_authority"], &ONDO_GM_PROGRAM)
}
pub fn ondo_user(user: &Pubkey, gm_mint: &Pubkey) -> Pubkey {
    pda(
        &[b"OndoUser", user.as_ref(), gm_mint.as_ref()],
        &ONDO_GM_PROGRAM,
    )
}
pub fn token_limit(gm_mint: &Pubkey) -> Pubkey {
    pda(&[b"token", gm_mint.as_ref()], &ONDO_GM_PROGRAM)
}
pub fn sanity_check(gm_mint: &Pubkey) -> Pubkey {
    pda(&[b"sanity_check", gm_mint.as_ref()], &ONDO_GM_PROGRAM)
}
pub fn attestation_account(id: &[u8; 16]) -> Pubkey {
    pda(&[b"attestation_id", id], &ONDO_GM_PROGRAM)
}
pub fn whitelist(user: &Pubkey) -> Pubkey {
    pda(&[b"whitelist", user.as_ref()], &ONDO_GM_PROGRAM)
}
pub fn usdon_manager_state() -> Pubkey {
    pda(&[b"usdon_manager"], &ONDO_GM_PROGRAM)
}
pub fn gmtoken_manager_state() -> Pubkey {
    pda(&[b"gmtoken_manager"], &ONDO_GM_PROGRAM)
}
pub fn gm_event_authority() -> Pubkey {
    pda(&[b"__event_authority"], &ONDO_GM_PROGRAM)
}
pub fn usdc_ata(owner: &Pubkey) -> Pubkey {
    get_associated_token_address_with_program_id(owner, &USDC_MINT, &TOKEN_PROGRAM)
}
pub fn token_2022_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, &TOKEN_2022_PROGRAM)
}

// ---- Instructions -----------------------------------------------------------------------------

/// The 14 accounts shared by `flashloan_borrow` and `flashloan_payback`.
pub fn flashloan_accounts(signer: &Pubkey, mint: &Pubkey) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(*signer, true),
        AccountMeta::new(flashloan_admin(), false),
        AccountMeta::new(
            get_associated_token_address_with_program_id(signer, mint, &TOKEN_PROGRAM),
            false,
        ),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(token_reserve(mint), false),
        AccountMeta::new(flashloan_borrow_position(mint), false),
        AccountMeta::new_readonly(rate_model(mint), false),
        AccountMeta::new(liquidity_vault(mint), false),
        AccountMeta::new_readonly(liquidity(), false),
        AccountMeta::new_readonly(LIQUIDITY_PROGRAM, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM, false),
        AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new_readonly(SYSVAR_INSTRUCTIONS, false),
    ]
}

fn flashloan_ix(disc: [u8; 8], signer: &Pubkey, mint: &Pubkey, amount: u64) -> Instruction {
    let mut data = disc.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: FLASHLOAN_PROGRAM,
        accounts: flashloan_accounts(signer, mint),
        data,
    }
}

pub fn flashloan_borrow_ix(signer: &Pubkey, mint: &Pubkey, amount: u64) -> Instruction {
    flashloan_ix(FLASHLOAN_BORROW_DISC, signer, mint, amount)
}

pub fn flashloan_payback_ix(signer: &Pubkey, mint: &Pubkey, amount: u64) -> Instruction {
    flashloan_ix(FLASHLOAN_PAYBACK_DISC, signer, mint, amount)
}

/// The 25 accounts shared by `mint_with_usdc` and `redeem_for_usdc`.
pub fn ondo_usdc_swap_accounts(
    user: &Pubkey,
    gm_mint: &Pubkey,
    attestation_id: &[u8; 16],
) -> Vec<AccountMeta> {
    let manager = usdon_manager_state();
    vec![
        AccountMeta::new(*user, true),
        AccountMeta::new(*gm_mint, false),
        AccountMeta::new_readonly(gm_mint_authority(), false),
        AccountMeta::new(ondo_user(user, gm_mint), false),
        AccountMeta::new(token_limit(gm_mint), false),
        AccountMeta::new(sanity_check(gm_mint), false),
        AccountMeta::new(token_2022_ata(user, gm_mint), false),
        AccountMeta::new(attestation_account(attestation_id), false),
        AccountMeta::new_readonly(whitelist(user), false),
        AccountMeta::new_readonly(TOKEN_2022_PROGRAM, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM, false),
        AccountMeta::new_readonly(PYTH_USDC_PRICE_UPDATE, false),
        AccountMeta::new(
            get_associated_token_address_with_program_id(&manager, &USDC_MINT, &TOKEN_PROGRAM),
            false,
        ),
        AccountMeta::new(token_2022_ata(&manager, &USDON_MINT), false),
        AccountMeta::new_readonly(USDC_MINT, false),
        AccountMeta::new(usdc_ata(user), false),
        AccountMeta::new(USDON_MINT, false),
        AccountMeta::new(token_2022_ata(user, &USDON_MINT), false),
        AccountMeta::new_readonly(manager, false),
        AccountMeta::new(gmtoken_manager_state(), false),
        AccountMeta::new_readonly(SYSVAR_INSTRUCTIONS, false),
        AccountMeta::new_readonly(gm_event_authority(), false),
        AccountMeta::new_readonly(ONDO_GM_PROGRAM, false),
    ]
}

fn ondo_ix_data(att: &Attestation) -> Vec<u8> {
    let disc = match att.side {
        FlashSide::Mint => MINT_WITH_USDC_DISC,
        FlashSide::Redeem => REDEEM_FOR_USDC_DISC,
    };
    let mut data = disc.to_vec();
    data.extend_from_slice(&att.id);
    data.extend_from_slice(&att.price.to_le_bytes());
    data.extend_from_slice(&att.amount.to_le_bytes());
    data.extend_from_slice(&att.expiration.to_le_bytes());
    data
}

/// `mint_with_usdc` or `redeem_for_usdc`, depending on the attestation side.
pub fn ondo_usdc_swap_ix(user: &Pubkey, gm_mint: &Pubkey, att: &Attestation) -> Instruction {
    Instruction {
        program_id: ONDO_GM_PROGRAM,
        accounts: ondo_usdc_swap_accounts(user, gm_mint, &att.id),
        data: ondo_ix_data(att),
    }
}

/// keccak256 over the 137-byte quote the Ondo program reconstructs on-chain.
pub fn attestation_digest(
    chain_id: &Pubkey,
    att: &Attestation,
    user: &Pubkey,
    asset: &Pubkey,
) -> [u8; 32] {
    let side = match att.side {
        FlashSide::Mint => SIDE_BUY,
        FlashSide::Redeem => SIDE_SELL,
    };
    keccak::hashv(&[
        chain_id.as_ref(),
        &att.id,
        &[side],
        user.as_ref(),
        asset.as_ref(),
        &att.price.to_be_bytes(),
        &att.amount.to_be_bytes(),
        &att.expiration.to_be_bytes(),
    ])
    .to_bytes()
}

/// Recover the Ethereum address that signed `digest`. Must equal the on-chain attestation signer
/// before an attestation is put in a transaction; a mismatch means the encoding is wrong.
/// The secp256k1 precompile keccak-hashes the message it is given, so the signature is over
/// `keccak256(digest)`, not over `digest` itself.
pub fn recover_signer(att: &Attestation, digest: &[u8; 32]) -> Result<[u8; 20]> {
    let signed_hash = keccak::hash(digest).to_bytes();
    let pubkey = solana_sdk::secp256k1_recover::secp256k1_recover(
        &signed_hash,
        att.recovery_id,
        &att.signature,
    )
    .map_err(|e| anyhow!("secp256k1 recover failed: {e:?}"))?;
    let hash = keccak::hash(&pubkey.to_bytes()).to_bytes();
    Ok(hash[12..].try_into().expect("20 bytes"))
}

/// The secp256k1 precompile instruction the Ondo program expects immediately before its own.
/// `own_index` is this instruction's position in the transaction; the offsets reference it.
pub fn secp256k1_ix(
    eth_address: &[u8; 20],
    att: &Attestation,
    digest: &[u8; 32],
    own_index: u8,
) -> Instruction {
    const ETH_OFFSET: u16 = 12;
    const SIG_OFFSET: u16 = 32;
    const MSG_OFFSET: u16 = 97;
    let mut data = Vec::with_capacity(129);
    data.push(1u8);
    data.extend_from_slice(&SIG_OFFSET.to_le_bytes());
    data.push(own_index);
    data.extend_from_slice(&ETH_OFFSET.to_le_bytes());
    data.push(own_index);
    data.extend_from_slice(&MSG_OFFSET.to_le_bytes());
    data.extend_from_slice(&32u16.to_le_bytes());
    data.push(own_index);
    data.extend_from_slice(eth_address);
    data.extend_from_slice(&att.signature);
    data.push(att.recovery_id);
    data.extend_from_slice(digest);
    Instruction {
        program_id: SECP256K1_PROGRAM,
        accounts: vec![],
        data,
    }
}

/// Wrap the RFQ's instruction list `[cu_price, cu_limit, ...rest with the fill...]` into a
/// flash-loan fill. Compute-budget instructions stay at 0 and 1 (the RFQ patches index 1). The
/// amount borrowed and repaid is the fill's USDC leg, read from the fill instruction itself.
pub fn wrap(
    core: Vec<Instruction>,
    maker: &Pubkey,
    gm_mint: &Pubkey,
    exp: &FlashFillExpectation,
) -> Result<Vec<Instruction>> {
    ensure!(
        core.len() >= 3
            && core[..2]
                .iter()
                .all(|ix| ix.program_id == solana_sdk::compute_budget::id()),
        "expected the two compute budget instructions first"
    );
    let fill = core
        .iter()
        .find(|ix| ix.program_id == crate::order_engine::ID)
        .context("no fill instruction to wrap")?;
    let args =
        <crate::order_engine::client::args::Fill as anchor_lang::AnchorDeserialize>::deserialize(
            &mut fill.data.get(8..).unwrap_or_default(),
        )
        .context("fill args")?;
    let att = &exp.attestation;
    let leg = usdc_leg(att.side, args.input_amount, args.output_amount);
    let digest = attestation_digest(&exp.chain_id, att, maker, gm_mint);
    let mut out = Vec::with_capacity(core.len() + 4);
    let mut core = core.into_iter();
    out.push(core.next().unwrap());
    out.push(core.next().unwrap());
    out.push(flashloan_borrow_ix(maker, &USDC_MINT, leg));
    let ondo = |out: &mut Vec<Instruction>| {
        out.push(secp256k1_ix(
            &exp.attestation_signer,
            att,
            &digest,
            out.len() as u8,
        ));
        out.push(ondo_usdc_swap_ix(maker, gm_mint, att));
    };
    match att.side {
        FlashSide::Mint => {
            ondo(&mut out);
            out.extend(core);
        }
        FlashSide::Redeem => {
            out.extend(core);
            ondo(&mut out);
        }
    }
    out.push(flashloan_payback_ix(maker, &USDC_MINT, leg));
    Ok(out)
}

/// Whether a message invokes the flashloan program (program ids are always static keys).
pub fn message_uses_flash_loan(message: &solana_sdk::message::VersionedMessage) -> bool {
    message.static_account_keys().contains(&FLASHLOAN_PROGRAM)
}

/// Accounts worth putting in an address lookup table: everything static across fills for these
/// GM mints and makers. Invoked program ids are listed too; the compiler keeps them static anyway.
pub fn lookup_accounts(gm_mints: &[Pubkey], makers: &[Pubkey]) -> Vec<Pubkey> {
    let manager = usdon_manager_state();
    let mut keys = vec![
        FLASHLOAN_PROGRAM,
        LIQUIDITY_PROGRAM,
        ONDO_GM_PROGRAM,
        SECP256K1_PROGRAM,
        SYSVAR_INSTRUCTIONS,
        SYSTEM_PROGRAM,
        TOKEN_PROGRAM,
        TOKEN_2022_PROGRAM,
        ASSOCIATED_TOKEN_PROGRAM,
        USDC_MINT,
        USDON_MINT,
        PYTH_USDC_PRICE_UPDATE,
        flashloan_admin(),
        liquidity(),
        token_reserve(&USDC_MINT),
        rate_model(&USDC_MINT),
        flashloan_borrow_position(&USDC_MINT),
        liquidity_vault(&USDC_MINT),
        gm_mint_authority(),
        manager,
        gmtoken_manager_state(),
        gm_event_authority(),
        get_associated_token_address_with_program_id(&manager, &USDC_MINT, &TOKEN_PROGRAM),
        token_2022_ata(&manager, &USDON_MINT),
    ];
    for mint in gm_mints {
        keys.extend([*mint, token_limit(mint), sanity_check(mint)]);
    }
    for maker in makers {
        keys.extend([
            whitelist(maker),
            usdc_ata(maker),
            token_2022_ata(maker, &USDON_MINT),
        ]);
        for mint in gm_mints {
            keys.extend([ondo_user(maker, mint), token_2022_ata(maker, mint)]);
        }
    }
    keys
}

// ---- Validation (maker side) ------------------------------------------------------------------

/// Tracks the wrapper instructions while `validate_fill_sanitized_message` walks the message.
#[derive(Default)]
pub struct FlashTracker {
    borrow: Option<usize>,
    payback: Option<usize>,
    secp: Option<usize>,
    ondo: Option<usize>,
}

fn same_accounts(actual: &[BorrowedAccountMeta], expected: &[AccountMeta]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(a, e)| {
            *a.pubkey == e.pubkey
                && (!e.is_writable || a.is_writable)
                && (!e.is_signer || a.is_signer)
        })
}

impl FlashTracker {
    /// The GM mint is whichever side of the order is not USDC.
    pub fn gm_mint(order: &crate::fill::Order, side: FlashSide) -> Result<Pubkey> {
        let (usdc, gm) = match side {
            FlashSide::Mint => (order.input_mint, order.output_mint),
            FlashSide::Redeem => (order.output_mint, order.input_mint),
        };
        ensure!(usdc == USDC_MINT, "flash fill must be against USDC");
        ensure!(
            gm != USDC_MINT,
            "flash fill needs a GM mint on the other side"
        );
        Ok(gm)
    }

    pub fn observe(
        &mut self,
        index: usize,
        ix: &BorrowedInstruction,
        exp: &FlashFillExpectation,
        order: &crate::fill::Order,
    ) -> Result<()> {
        let program_id = *ix.program_id;
        let att = &exp.attestation;
        let gm_mint = Self::gm_mint(order, att.side)?;
        let leg = usdc_leg(att.side, order.in_amount, order.out_amount);
        if program_id == FLASHLOAN_PROGRAM {
            ensure!(
                same_accounts(&ix.accounts, &flashloan_accounts(&order.maker, &USDC_MINT)),
                "flashloan accounts differ from the maker's USDC flash loan"
            );
            if ix.data == flashloan_borrow_ix(&order.maker, &USDC_MINT, leg).data {
                ensure!(
                    self.borrow.replace(index).is_none(),
                    "duplicate flashloan borrow"
                );
            } else if ix.data == flashloan_payback_ix(&order.maker, &USDC_MINT, leg).data {
                ensure!(
                    self.payback.replace(index).is_none(),
                    "duplicate flashloan payback"
                );
            } else {
                bail!("unexpected flashloan instruction or amount");
            }
        } else if program_id == SECP256K1_PROGRAM {
            let digest = attestation_digest(&exp.chain_id, att, &order.maker, &gm_mint);
            let expected = secp256k1_ix(&exp.attestation_signer, att, &digest, index as u8);
            ensure!(
                ix.accounts.is_empty() && ix.data == expected.data,
                "secp256k1 instruction does not match the attestation"
            );
            ensure!(
                self.secp.replace(index).is_none(),
                "duplicate secp256k1 instruction"
            );
        } else if program_id == ONDO_GM_PROGRAM {
            ensure!(
                same_accounts(
                    &ix.accounts,
                    &ondo_usdc_swap_accounts(&order.maker, &gm_mint, &att.id)
                ),
                "Ondo accounts differ from the maker's mint/redeem"
            );
            ensure!(
                ix.data == ondo_ix_data(att),
                "Ondo instruction data differs from the attestation"
            );
            ensure!(
                self.ondo.replace(index).is_none(),
                "duplicate Ondo instruction"
            );
        } else {
            bail!("Unexpected program id {program_id}");
        }
        Ok(())
    }

    pub fn finish(
        &self,
        fill_index: usize,
        instruction_count: usize,
        exp: &FlashFillExpectation,
        order: &crate::fill::Order,
    ) -> Result<()> {
        let att = &exp.attestation;
        let borrow = self
            .borrow
            .ok_or_else(|| anyhow!("missing flashloan borrow"))?;
        let payback = self
            .payback
            .ok_or_else(|| anyhow!("missing flashloan payback"))?;
        let secp = self
            .secp
            .ok_or_else(|| anyhow!("missing secp256k1 instruction"))?;
        let ondo = self
            .ondo
            .ok_or_else(|| anyhow!("missing Ondo instruction"))?;
        ensure!(
            borrow < fill_index && fill_index < payback,
            "borrow must precede the fill and payback follow it"
        );
        ensure!(
            payback + 1 == instruction_count,
            "flashloan payback must be the last instruction"
        );
        ensure!(
            secp + 1 == ondo,
            "secp256k1 instruction must immediately precede the Ondo instruction"
        );
        ensure!(
            order.expire_at <= att.expiration,
            "fill outlives the attestation"
        );
        match att.side {
            FlashSide::Mint => {
                ensure!(ondo < fill_index, "mint must precede the fill");
                ensure!(
                    att.amount >= order.out_amount,
                    "attestation mints less than the fill delivers"
                );
            }
            FlashSide::Redeem => {
                ensure!(ondo > fill_index, "redeem must follow the fill");
                ensure!(
                    att.amount <= order.in_amount,
                    "attestation redeems more than the taker delivers"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn pk(s: &str) -> Pubkey {
        Pubkey::from_str(s).unwrap()
    }
    fn hex16(s: &str) -> [u8; 16] {
        let v: Vec<u8> = (0..32)
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect();
        v.try_into().unwrap()
    }
    fn hex_vec(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Mainnet tx 2uYghATvDcm92p7swkV1YkFd6YGGzzZQsHhoUnwcAWrdM13RshUvrKieBRf8Dg4mcHgTwLWH9EeZXRRLerRo2fT7
    fn real_mint() -> (Pubkey, Pubkey, Attestation) {
        let user = pk("DSqMPMsMAbEJVNuPKv1ZFdzt6YvJaDPDddfeW7ajtqds");
        let mint = pk("14kLsQVmc64qZexYuR4XGop9y8BeMkd77pJUm1Rhondo");
        let sig = hex_vec("6e21ca1dcffeedfa3826f982ed471d75a438f8fe486d18c2581355f11fc927153b977d186cd52811499965ba6824e6edf7d052e842a491e19537cca2b1771228");
        let att = Attestation {
            id: hex16("f12d2fc0f52d49e789a14ce6b592f5db"),
            side: FlashSide::Mint,
            price: 15_457_725_000,
            amount: 323_121_628,
            expiration: 1_790_071_881,
            signature: sig.try_into().unwrap(),
            recovery_id: 0,
        };
        (user, mint, att)
    }

    #[test]
    fn flashloan_accounts_match_mainnet_borrow() {
        // Mainnet tx 2pHeKYb4QAKyFz6MsXFCaZe5FwfKdznSdFBjYSv1vD36h66Jp4nPQ74JDvoCikwVTwoobjo9ZXfYNvoVTW3tziMa
        let signer = pk("Dz744q23G6vcMtuZnRMUDyP63gu4GGWpuawKRLmua8Ar");
        let expected = [
            "Dz744q23G6vcMtuZnRMUDyP63gu4GGWpuawKRLmua8Ar",
            "ALXWtv2P4GqH1B7Lq731joag52yRBRqmHV4naiXPTYWL",
            "3hUp3vqjUTuFTkFfQ1uXTDcr3QktVNdqF2W9eqGkg4PP",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "94vK29npVbyRHXH63rRcTiSr26SFhrQTzbpNJuhQEDu",
            "J9dyC4pBTBPvzzPh7J9rhFhg8RvgerDNKkUH9kEwGMsj",
            "5pjzT5dFTsXcwixoab1QDLvZQvpYJxJeBphkyfHGn688",
            "BmkUoKMFYBxNSzWXyUjyMJjMAaVz4d8ZnxwwmhDCUXFB",
            "7s1da8DduuBFqGra5bJBjpnvL5E9mGzCuMk1Qkh4or2Z",
            "jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC",
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            "11111111111111111111111111111111",
            "Sysvar1nstructions1111111111111111111111111",
        ];
        let actual: Vec<String> = flashloan_accounts(&signer, &USDC_MINT)
            .iter()
            .map(|a| a.pubkey.to_string())
            .collect();
        assert_eq!(actual, expected);
        let ix = flashloan_borrow_ix(&signer, &USDC_MINT, 1);
        assert_eq!(ix.data.len(), 16);
        assert_eq!(&ix.data[..8], &FLASHLOAN_BORROW_DISC);
    }

    #[test]
    fn ondo_accounts_match_mainnet_mint() {
        let (user, mint, att) = real_mint();
        let expected = [
            "DSqMPMsMAbEJVNuPKv1ZFdzt6YvJaDPDddfeW7ajtqds",
            "14kLsQVmc64qZexYuR4XGop9y8BeMkd77pJUm1Rhondo",
            "9foMHsSDq7nMg4WPusSz9eY7tyxyukqborA8GyU5cUxD",
            "6JMbnm3VhYiTvH6LYG2cYj2meEowtJfvwLqy4BAwqkVP",
            "3hc5E13F7kYFDsSaP3p1Egu2fFpPUhALVzpYJLJFWasS",
            "4fDnC4jD1jVm8Zc73AUFkjELUiZ3o926uia67Jn5odiU",
            "4ML255pA492WPN14JD4GSoWJgJUUPdCyytDvSdSiMMmJ",
            "HNbJVFppQUtxuZxKHks7y2WuDputj2FvZQd66iHbhsqb",
            "C2moE8vPUVsuVVWQGU9gM5bf8dPy9kYivd3hHCyDZjcv",
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
            "11111111111111111111111111111111",
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX",
            "21MqWTzhM5BHykeXK264D8AQjwWrdi6vurbX4epaovPU",
            "CKeSkoxSU8uvyYU5Hoz3TxdPwQpYJDnApW8jNv57jvzs",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "8raAdppi7DegxmkdWZVEtH3axsSSF9oRM43DrHgF7i3e",
            "ZPFtoCe7WWqG4N3ZFRccS8T9SMBeHsd1Vmgv2i7ondo",
            "AKWFr71wjg1GJWjwW6ArS9Zv87FoVDgjqYDRMH7wFgux",
            "CKW82CT7QtJUDkxCa6sSHZ3UXvUSu6EuGDXUtKyXgn67",
            "3cKyGYojYbFxeUnBf5xak3cTpaxmYuzpFbSnP9n3kyui",
            "Sysvar1nstructions1111111111111111111111111",
            "6ipdj6w8PRZqwESgGyDnmJr2vcwu51bKNFk7yd2K96Bz",
            "XzTT4XB8m7sLD2xi6snefSasaswsKCxx5Tifjondogm",
        ];
        let actual: Vec<String> = ondo_usdc_swap_accounts(&user, &mint, &att.id)
            .iter()
            .map(|a| a.pubkey.to_string())
            .collect();
        assert_eq!(actual, expected);
        let ix = ondo_usdc_swap_ix(&user, &mint, &att);
        assert_eq!(
            ix.data,
            hex_vec("801c85ad478eb9cef12d2fc0f52d49e789a14ce6b592f5db482a5a9903000000dc714213000000004954b26a00000000")
        );
    }

    #[test]
    fn digest_and_secp_instruction_match_mainnet() {
        let (user, mint, att) = real_mint();
        let digest = attestation_digest(&MAINNET_CHAIN_ID, &att, &user, &mint);
        assert_eq!(
            hex_vec("81f148978e028828c4cb1da870fba0cbfe2304b1f3d1c461a0d0da7ef08607f8"),
            digest
        );
        assert_eq!(
            recover_signer(&att, &digest).unwrap(),
            ONDO_ATTESTATION_SIGNER
        );
        let mut wrong = att.clone();
        wrong.price += 1;
        let wrong_digest = attestation_digest(&MAINNET_CHAIN_ID, &wrong, &user, &mint);
        assert_ne!(
            recover_signer(&wrong, &wrong_digest).unwrap(),
            ONDO_ATTESTATION_SIGNER
        );
        // It was instruction 3 in that transaction.
        let ix = secp256k1_ix(&ONDO_ATTESTATION_SIGNER, &att, &digest, 3);
        let expected = format!(
            "01{}{}{}{}",
            "200003" /* sig offset 32, ix 3 */.to_owned() + "0c0003" /* eth offset 12, ix 3 */ + "6100" /* msg offset 97 */ + "2000" /* msg len 32 */ + "03",
            "ff2d04f6002c10eba702adafe2f43cac5996c477",
            "6e21ca1dcffeedfa3826f982ed471d75a438f8fe486d18c2581355f11fc927153b977d186cd52811499965ba6824e6edf7d052e842a491e19537cca2b177122800",
            "81f148978e028828c4cb1da870fba0cbfe2304b1f3d1c461a0d0da7ef08607f8"
        );
        assert_eq!(ix.data, hex_vec(&expected));
        assert_eq!(ix.data.len(), 129);

        // Redeem side: mainnet tx 2bsFdPLLYtu5FEr2vrZksQs1UcTsyiwAHVsUNWt9jkGdbFPT8eoe2Wprwyijq5HxpNewSh77NuspJAQNQP1BLqCr
        let redeem = Attestation {
            id: hex16("18b09fe0c02d4ee2ae8107c30cc93fdb"),
            side: FlashSide::Redeem,
            price: 1_766_235_002_000,
            amount: 23_548_000,
            expiration: 1_790_072_387,
            signature: [0; 64],
            recovery_id: 0,
        };
        let digest = attestation_digest(
            &MAINNET_CHAIN_ID,
            &redeem,
            &user,
            &pk("EJmUVvDqAdfH5zEohkdS4234bi3c6iunqEMobjmondo"),
        );
        assert_eq!(
            hex_vec("89bf29493e32e6fb82a3b63ad9ea0c78ef9067a8b9620c0d8e519eec142c9724"),
            digest
        );
    }

    #[test]
    fn parses_wire_payload() {
        let (_, _, att) = real_mint();
        let mut sig65 = att.signature.to_vec();
        sig65.push(27); // Ethereum v
        let wire = webhook_api::responses::FlashFill {
            attestation_id: "320578568903735945052344740413286053339".into(),
            side: FlashSide::Mint,
            price: "15457725000".into(),
            amount: "323121628".into(),
            expiration: 1_790_071_881,
            signature: BASE64_STANDARD.encode(&sig65),
        };
        assert_eq!(Attestation::try_from(&wire).unwrap(), att);
        // Ondo's documented example id is a version-4 UUID only when read big-endian.
        let example = webhook_api::responses::FlashFill {
            attestation_id: "229852750420835981756873903928305653446".into(),
            ..wire
        };
        let id = Attestation::try_from(&example).unwrap().id;
        assert_eq!(
            (id[6] >> 4, id[8] >> 6),
            (4, 0b10),
            "UUID version 4, RFC 4122 variant"
        );
        assert_eq!(lookup_accounts(&[USDC_MINT], &[]).len(), 27);
    }

    #[test]
    fn reads_the_live_flashloan_fee() {
        // flashloan_admin on mainnet, 2026-10-15: status 1 (active), fee 0.
        let admin = hex_vec("a2a12d1c835bca58fa20b7645eeeecdca805b93db8c46089cb858ff0d84fe5d0a478a3c4e18fe4950afe1e97e456cbb92e642a25b50ba6396ff248164bca746963a84592acd6ccc301000000b5d06a00000000000000000000000000ff");
        assert_eq!(flashloan_fee_bps(&admin).unwrap(), 0);
        let mut charged = admin.clone();
        charged[73] = 5;
        assert_eq!(flashloan_fee_bps(&charged).unwrap(), 5);
        assert!(flashloan_fee_bps(&admin[..74]).is_err());
    }

    fn fill_ix(
        maker: Pubkey,
        taker: Pubkey,
        input_mint: Pubkey,
        output_mint: Pubkey,
        in_amount: u64,
        out_amount: u64,
        expire_at: i64,
    ) -> Instruction {
        use anchor_lang::{InstructionData, ToAccountMetas};
        let program_for = |mint: &Pubkey| {
            if *mint == USDC_MINT {
                TOKEN_PROGRAM
            } else {
                TOKEN_2022_PROGRAM
            }
        };
        let ata = |owner: &Pubkey, mint: &Pubkey| {
            get_associated_token_address_with_program_id(owner, mint, &program_for(mint))
        };
        let mut data = crate::order_engine::client::args::Fill {
            input_amount: in_amount,
            output_amount: out_amount,
            expire_at,
        }
        .data();
        data.extend_from_slice(&[2, 0, 0]);
        Instruction {
            program_id: crate::order_engine::ID,
            accounts: crate::order_engine::client::accounts::Fill {
                taker,
                maker,
                taker_input_mint_token_account: Some(ata(&taker, &input_mint)),
                maker_input_mint_token_account: Some(ata(&maker, &input_mint)),
                taker_output_mint_token_account: Some(ata(&taker, &output_mint)),
                maker_output_mint_token_account: Some(ata(&maker, &output_mint)),
                input_mint,
                input_token_program: program_for(&input_mint),
                output_mint,
                output_token_program: program_for(&output_mint),
                system_program: SYSTEM_PROGRAM,
            }
            .to_account_metas(None),
            data,
        }
    }

    fn sanitize(maker: &Pubkey, ixs: &[Instruction]) -> solana_sdk::message::SanitizedMessage {
        use solana_sdk::message::{v0, VersionedMessage};
        let msg =
            v0::Message::try_compile(maker, ixs, &[], solana_sdk::hash::Hash::default()).unwrap();
        crate::transaction::versioned_message_to_sanitized_message(VersionedMessage::V0(msg))
            .unwrap()
    }

    #[test]
    fn wraps_and_validates_both_sides() {
        use crate::fill::{
            validate_fill_sanitized_message, validate_flash_fill_sanitized_message, Order,
        };
        use solana_sdk::compute_budget::ComputeBudgetInstruction;

        let maker = Pubkey::new_unique();
        let taker = Pubkey::new_unique();
        let (_, gm_mint, att) = real_mint();
        let order = |in_amount, input_mint, out_amount, output_mint, expire_at| Order {
            taker,
            maker,
            in_amount,
            input_mint,
            out_amount,
            output_mint,
            expire_at,
        };
        let cu = || {
            vec![
                ComputeBudgetInstruction::set_compute_unit_price(1_000),
                ComputeBudgetInstruction::set_compute_unit_limit(600_000),
            ]
        };
        let programs = |ixs: &[Instruction]| ixs.iter().map(|ix| ix.program_id).collect::<Vec<_>>();

        // Mint side: taker pays 5 USDC and receives the minted amount minus Jupiter's fee.
        let expire_at = att.expiration - 5;
        let mut core = cu();
        core.push(fill_ix(
            maker,
            taker,
            USDC_MINT,
            gm_mint,
            5_000_000,
            att.amount - 1,
            expire_at,
        ));
        let exp = FlashFillExpectation::mainnet(att.clone());
        let ixs = wrap(core, &maker, &gm_mint, &exp).unwrap();
        // The borrow and the payback are the 5 USDC the taker pays.
        assert_eq!(ixs[2], flashloan_borrow_ix(&maker, &USDC_MINT, 5_000_000));
        assert_eq!(ixs[6], flashloan_payback_ix(&maker, &USDC_MINT, 5_000_000));
        assert_eq!(
            programs(&ixs),
            [
                solana_sdk::compute_budget::id(),
                solana_sdk::compute_budget::id(),
                FLASHLOAN_PROGRAM,
                SECP256K1_PROGRAM,
                ONDO_GM_PROGRAM,
                crate::order_engine::ID,
                FLASHLOAN_PROGRAM
            ]
        );
        let msg = sanitize(&maker, &ixs);
        let ok = order(5_000_000, USDC_MINT, att.amount - 1, gm_mint, expire_at);
        let validated = validate_flash_fill_sanitized_message(&msg, ok, &exp).unwrap();
        assert_eq!(validated.compute_unit_limit, 600_000);
        // The plain validator still refuses the wrapper.
        assert!(validate_fill_sanitized_message(
            &msg,
            order(5_000_000, USDC_MINT, att.amount - 1, gm_mint, expire_at)
        )
        .is_err());
        // A borrow other than the USDC leg, amounts other than the fill's, a fill outliving the
        // attestation, a missing payback.
        let mut over_borrow = ixs.clone();
        over_borrow[2] = flashloan_borrow_ix(&maker, &USDC_MINT, 5_000_001);
        assert!(validate_flash_fill_sanitized_message(
            &sanitize(&maker, &over_borrow),
            order(5_000_000, USDC_MINT, att.amount - 1, gm_mint, expire_at),
            &exp
        )
        .is_err());
        assert!(validate_flash_fill_sanitized_message(
            &msg,
            order(4_000_000, USDC_MINT, att.amount - 1, gm_mint, expire_at),
            &exp
        )
        .is_err());
        let late = sanitize(&maker, &{
            let mut c = cu();
            c.push(fill_ix(
                maker,
                taker,
                USDC_MINT,
                gm_mint,
                5_000_000,
                att.amount - 1,
                att.expiration + 1,
            ));
            wrap(c, &maker, &gm_mint, &exp).unwrap()
        });
        assert!(validate_flash_fill_sanitized_message(
            &late,
            order(
                5_000_000,
                USDC_MINT,
                att.amount - 1,
                gm_mint,
                att.expiration + 1
            ),
            &exp
        )
        .is_err());
        let no_payback = sanitize(&maker, &ixs[..6]);
        assert!(validate_flash_fill_sanitized_message(
            &no_payback,
            order(5_000_000, USDC_MINT, att.amount - 1, gm_mint, expire_at),
            &exp
        )
        .is_err());

        // Redeem side: taker sells the GM amount, receives 4.9 USDC borrowed up front.
        let mut redeem = att.clone();
        redeem.side = FlashSide::Redeem;
        let mut core = cu();
        core.push(fill_ix(
            maker,
            taker,
            gm_mint,
            USDC_MINT,
            redeem.amount,
            4_900_000,
            expire_at,
        ));
        let exp = FlashFillExpectation::mainnet(redeem.clone());
        let ixs = wrap(core, &maker, &gm_mint, &exp).unwrap();
        // The borrow and the payback are the 4.9 USDC the taker receives.
        assert_eq!(ixs[2], flashloan_borrow_ix(&maker, &USDC_MINT, 4_900_000));
        assert_eq!(
            programs(&ixs),
            [
                solana_sdk::compute_budget::id(),
                solana_sdk::compute_budget::id(),
                FLASHLOAN_PROGRAM,
                crate::order_engine::ID,
                SECP256K1_PROGRAM,
                ONDO_GM_PROGRAM,
                FLASHLOAN_PROGRAM
            ]
        );
        let msg = sanitize(&maker, &ixs);
        validate_flash_fill_sanitized_message(
            &msg,
            order(redeem.amount, gm_mint, 4_900_000, USDC_MINT, expire_at),
            &exp,
        )
        .unwrap();
        // Redeeming more than the taker delivers is refused.
        let mut greedy = exp.clone();
        greedy.attestation.amount += 1;
        assert!(validate_flash_fill_sanitized_message(
            &msg,
            order(redeem.amount, gm_mint, 4_900_000, USDC_MINT, expire_at),
            &greedy
        )
        .is_err());
    }
}
