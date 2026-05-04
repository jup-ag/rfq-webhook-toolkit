//! Example: wrap a swap instruction through a Squads multisig, then unwrap a
//! confirmed transaction from mainnet to inspect its inner instructions.
//!
//! Run with:
//!   cargo run --example wrap_and_unwrap
//!
//! Requires mainnet RPC access (uses a real confirmed transaction).

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey,
    pubkey::Pubkey,
    signature::Signature,
};
use solana_transaction_status::UiTransactionEncoding;
use std::str::FromStr;

use squads_sdk::{
    build_squads_wrapped_transaction, unwrap_transaction, unwrap_transaction_with_account_keys,
    SquadsWrapConfig,
};

#[tokio::main]
async fn main() {
    // ── Part 1: Wrap ────────────────────────────────────────────────────
    //
    // Take a swap instruction and wrap it so it executes through a Squads
    // multisig vault via executeTransactionSyncV2.

    let rpc = RpcClient::new("https://api.mainnet-beta.solana.com".to_string());

    // In production these come from your Squads multisig account on-chain.
    let settings_pda = pubkey!("8f1s1b4Y3CVP9vA8QFf8m6v3oc7Q5Q8m2Un9u9A34M2T");
    let vault_pda = pubkey!("3q8J3wTVpd6fHiFcPfebP8Fd6hQfKd8QxJ5zhhWgE4n9");
    let member_a = pubkey!("Dk9EdQJk3JxR5aVdS3tDqQnBk7LfMoT1n7Vm5R4n4fq4");
    let member_b = pubkey!("4C58H5fm5P5k2p4A6HRo25ykoPS2atdx2myTaYF9E1f3");

    let config = SquadsWrapConfig {
        settings_pda,
        vault_pda,
        members: vec![member_a, member_b],
        threshold: 2,
    };

    // Whatever swap/transfer the vault needs to execute.
    let token_program = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    let swap_program = pubkey!("9xQeWvG816bUx9EPf2st4qGSe6P6xj6Yy7D6A6M6y8d");
    let user_ata = pubkey!("GDrB6xfg2s7zNBi8W6vX4NQAz3gU8GdU4cf9jXhVJzjP");

    let swap_ix = Instruction {
        program_id: swap_program,
        accounts: vec![
            AccountMeta::new(vault_pda, true), // vault is the "signer" — Squads CPI signs for it
            AccountMeta::new(user_ata, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };

    // Get a recent blockhash from RPC (needed for the transaction message).
    let recent_blockhash = rpc
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await
        .expect("failed to get blockhash")
        .0;

    // Wrap it. The result is a VersionedTransaction with null signatures —
    // members sign it before submitting.
    let wrapped_tx = build_squads_wrapped_transaction(
        &[swap_ix],
        &config,
        recent_blockhash,
        400_000, // compute unit limit
        500_000, // compute unit price (micro-lamports)
    )
    .expect("failed to wrap transaction");

    println!("=== Wrap ===");
    println!(
        "  signatures: {} (null until members sign)",
        wrapped_tx.signatures.len()
    );

    // You can unwrap your own wrapped tx without ALTs (no lookup tables used).
    let unwrapped = unwrap_transaction(&wrapped_tx).expect("failed to unwrap");
    println!(
        "  round-trip inner instructions: {}",
        unwrapped.instructions.len()
    );
    println!("  inner program: {}", unwrapped.instructions[0].program_id);
    println!("  inner data: {:?}", unwrapped.instructions[0].data);

    // ── Part 2: Unwrap a real mainnet transaction (with ALTs) ───────────
    //
    // Real Squads transactions usually use Address Lookup Tables. To unwrap
    // them you need the full resolved account key list from RPC.

    // A real Squads-wrapped SOL→USDC swap on mainnet.
    let tx_sig = Signature::from_str(
        "5kdWZuVbbfY7vPFH78ueRczphwbo7c6RiywaMcwgcAZyr5ZM29jSHudZFx9VMNRLZkKCqQZWPEzd1uaPWVyyM855",
    )
    .unwrap();

    println!("\n=== Unwrap mainnet tx ===");
    println!("  signature: {tx_sig}");

    // Fetch the confirmed transaction. maxSupportedTransactionVersion=0
    // is required for V0 (versioned) transactions.
    let tx_response = rpc
        .get_transaction_with_config(
            &tx_sig,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            },
        )
        .await
        .expect("failed to fetch transaction");

    // The RPC response gives us the full resolved account key list:
    //   static keys ++ ALT writable keys ++ ALT readonly keys
    // This is what unwrap_transaction_with_account_keys needs.
    let ui_tx = tx_response
        .transaction
        .transaction
        .decode()
        .expect("failed to decode transaction");

    let meta = tx_response
        .transaction
        .meta
        .expect("transaction has no meta");

    // Build the full account key list: static keys + loaded ALT addresses
    let mut account_keys: Vec<Pubkey> = ui_tx.message.static_account_keys().to_vec();
    if let solana_transaction_status::option_serializer::OptionSerializer::Some(loaded) =
        meta.loaded_addresses
    {
        for addr in &loaded.writable {
            account_keys.push(Pubkey::from_str(addr).expect("invalid writable ALT pubkey"));
        }
        for addr in &loaded.readonly {
            account_keys.push(Pubkey::from_str(addr).expect("invalid readonly ALT pubkey"));
        }
    }

    println!(
        "  account keys: {} static + {} from ALTs = {} total",
        ui_tx.message.static_account_keys().len(),
        account_keys.len() - ui_tx.message.static_account_keys().len(),
        account_keys.len(),
    );

    // Now unwrap with the full key list.
    let unwrapped =
        unwrap_transaction_with_account_keys(&ui_tx, &account_keys).expect("failed to unwrap");

    println!("  settings_pda: {}", unwrapped.settings_pda);
    println!("  members: {:?}", unwrapped.members);
    println!("  num_signers: {}", unwrapped.num_signers);
    println!("  compute_unit_limit: {}", unwrapped.compute_unit_limit);
    println!("  compute_unit_price: {}", unwrapped.compute_unit_price);
    println!("  inner instructions:");
    for (i, ix) in unwrapped.instructions.iter().enumerate() {
        println!(
            "    ix[{}]: program={}, accounts={}, data_len={}",
            i,
            ix.program_id,
            ix.accounts.len(),
            ix.data.len(),
        );
    }
}
