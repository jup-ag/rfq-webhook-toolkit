//! Example: wrap a swap instruction through a Squads multisig, then unwrap it
//! to inspect the inner instructions.
//!
//! Run with:
//!   cargo run --example wrap_and_unwrap
//!
//! Requires mainnet RPC access (for the blockhash).

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

use squads_sdk::{build_squads_wrapped_transaction, unwrap_transaction, SquadsWrapConfig};

#[tokio::main]
async fn main() {
    // Take a swap instruction and wrap it so it executes through a Squads
    // multisig vault via executeTransactionSyncV2. The output is self-contained
    // (no address lookup tables) — anyone can verify it offline.
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
        fee_payer: None,
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

    let recent_blockhash = rpc
        .get_latest_blockhash()
        .await
        .expect("failed to get blockhash");

    // Wrap it. The result has null signatures — members sign it before submitting.
    let wrapped_tx = build_squads_wrapped_transaction(
        &[swap_ix],
        &config,
        recent_blockhash,
        400_000, // compute unit limit
        500_000, // compute unit price (micro-lamports)
    )
    .expect("failed to wrap transaction");

    println!(
        "wrapped: {} signatures (null until members sign)",
        wrapped_tx.signatures.len()
    );

    // Unwrap to verify the inner instruction round-trips cleanly.
    let unwrapped = unwrap_transaction(&wrapped_tx).expect("failed to unwrap");
    let inner = &unwrapped.instructions[0];

    println!("unwrapped:");
    println!("  inner instructions: {}", unwrapped.instructions.len());
    println!("  inner program:      {}", inner.program_id);
    println!("  inner data:         {:?}", inner.data);
    println!("  settings_pda:       {}", unwrapped.settings_pda);
    println!("  members:            {:?}", unwrapped.members);
    println!("  num_signers:        {}", unwrapped.num_signers);
    println!("  compute_unit_limit: {}", unwrapped.compute_unit_limit);
    println!("  compute_unit_price: {}", unwrapped.compute_unit_price);
}
