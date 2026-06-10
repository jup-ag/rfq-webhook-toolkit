# jupnet-order-engine

Jupnet RFQ order-engine program (Pinocchio-based). Same on-chain semantics as the Anchor `order-engine` in `../order-engine`, smaller binary (~36 KB), incompatible wire format (1-byte tag + 1-byte presence bitmap + packed args — see `src/processor/fill.rs` for the layout).

**Program id (devnet):** `473HkSFbCjEDmjecgXsuGXt25VkJpUZj7gAxywVpU46c`

## Build the on-chain binary

```bash
cargo build-sbf \
  --manifest-path programs/jupnet-order-engine/Cargo.toml \
  --features bpf-entrypoint
```

Output: `target/deploy/jupnet_order_engine.so` (+ keypair).

`bpf-entrypoint` is required — the entrypoint, allocator, and panic handler only compile in when that feature is on, so the crate stays linkable as a library otherwise.

With the same `target/deploy/jupnet_order_engine-keypair.json` present, subsequent `program deploy` calls become upgrades (provided you hold the upgrade authority).

## Testing

There is no local `cargo test` harness. The upstream SPL Token program inside `solana-program-test` uses u64 amounts in `Transfer`/`TransferChecked` data, which is incompatible with the Jupnet 32-byte format the program now emits.
