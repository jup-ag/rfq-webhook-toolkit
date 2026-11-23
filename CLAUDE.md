# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

Integration toolkit for market makers (MMs) joining Jupiter RFQ. It contains the on-chain
Order Engine program that settles RFQ fills, the off-chain SDK that MMs use to *validate* fill
transactions before signing them, the webhook request/response types, a runnable reference
webhook server, and TS test suites that exercise a real webhook.

The security-critical asset is the validation logic: an MM signs a transaction it did not build,
so `order-engine-sdk/src/fill.rs` is what stands between a maker's funds and a malicious
transaction.

## Commands

Rust toolchain is pinned via `rust-toolchain.toml`.

```bash
cargo build --release                      # what CI builds
cargo fmt -- --check                       # CI gate
cargo clippy --all-targets -- -D warnings  # CI gate; warnings are errors
make cargofix                              # cargo fix + fmt --all + clippy --fix

cargo test --workspace
cargo test -p order-engine-sdk                                  # off-chain validation unit tests
cargo test -p order-engine --test test_fill                     # on-chain tests (solana-program-test)
cargo test -p order-engine-sdk fill::tests::<name> -- --nocapture  # a single test
```

Reference server and TS suites (Makefile targets — note the target is `run-example-server`,
not the `run-server-example` the README mentions):

```bash
make run-example-server            # serves on :8080, Swagger UI at /swagger-ui/
make prepare-tests                 # pnpm install in tests/
make run-acceptance-tests-against-sample-server   # boots the example server, runs acceptance suite
WEBHOOK_URL=<url> WEBHOOK_API_KEY=<key> make run-acceptance-tests
make update-openapi-spec           # regenerates openapi/openapi.json from the running server
```

`make run-integration-tests` hits Jupiter's pre-prod edge and **performs a real mainnet swap** —
requires a funded `TAKER_KEYPAIR` and a registered `WEBHOOK_ID`. Do not run it casually.

## Workspace layout and how the pieces relate

Cargo workspace (`members = ["server-example", "webhook-api", "programs/*", "order-engine-sdk", "squads-sdk"]`).

- **`programs/order-engine`** — Anchor program with a single `fill` instruction.
- **`order-engine-sdk`** — off-chain counterpart. `declare_program!(order_engine)` generates types
  from `idls/order_engine.json`, so **regenerate that IDL when the program's interface changes**.
- **`webhook-api`** — the wire contract only (`QuoteRequest`/`QuoteResponse`, `SwapRequest`/
  `SwapResponse`, enums), `camelCase` serde + `utoipa::ToSchema` on every field. Amounts are
  `String`, not integers. `webhook-api/src/mod.rs` duplicates `lib.rs`; `lib.rs` is the real root.
- **`server-example`** — axum reference webhook (`/quote`, `/swap`, `/tokens`, plus a non-spec
  `/health`).
- **`squads-sdk`** — wrap/unwrap RFQ transactions so a Squads multisig vault executes them via
  `executeTransactionSyncV2`.
- **`tests/`** — Vitest + pnpm. `suites/acceptance` mimics Jupiter calling a webhook (safe, local);
  `suites/integration` runs against edge and swaps for real.

## Parsing discriminators

in `order-engine-sdk*` **do not** hand-roll discriminator extraction with `.split_at(8)`, `.split_first(…)`,
`data[..8]`, `data[0..8]`, `.get(0)`, or `.first()`. Every one of those either panics on a short
buffer or silently accepts a truncated one, on data that arrives from an untrusted transaction.

Instead use `order_engine_sdk::parse_util`:

```rust
use crate::parse_util::{split_disc1byte_and_bytes, split_disc8bytes_and_bytes};

let (discriminator, args) = split_disc8bytes_and_bytes(data)?;  // anchor-style 8-byte disc
let (discriminator, args) = split_disc1byte_and_bytes(data)?;   // 1-byte disc (spl-token, lighthouse)
```

Both return `Result<(&[u8; N], &[u8]), ParseError>` — length-checked, no panic, and the fixed-size
array lets you compare against a discriminator constant without a length assertion. See the call
sites in `order-engine-sdk/src/fill.rs`. If you need a width these don't cover, add a
`split_discNbytes_and_bytes` to `parse_util.rs` with tests rather than inlining the slicing.

`squads-sdk` is allowed to use low-level access.

## Clippy conventions

Lints are configured workspace-wide in `Cargo.toml` and match the codebase's style deliberately.
Raise conversation if clippy rules are too strict or too lax, but do not disable them lightly.+
