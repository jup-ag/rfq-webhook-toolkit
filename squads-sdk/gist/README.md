# Squads Reference Gists

Reference code from [Orion (0xRigel)](https://gist.github.com/0xRigel-squads/9e334eab2a598f5c306a6a85fdcbcc31), a Squads V5 smart-account-program developer. These snippets show how Squads internally wraps transactions for `executeTransactionSync` / `executeTransactionSyncV2`.

## Files

- **`sync.rs`** — Entry point for building a wrapped Squads instruction. Takes a `SyncTransactionMeta` (vault, settings PDA, members, inner instructions) and produces a single `executeTransactionSync` instruction.
- **`helpers.rs`** — Core compilation logic. Deduplicates and sorts accounts, compiles instructions into Squads' `CustomCompiledInstruction` format, serializes them into the on-chain binary payload, and computes Anchor discriminators.

## How our SDK relates

Our `squads-sdk` reimplements this wrapping logic with additional features:
- Base64 transaction wrap/unwrap for API integration
- Compute budget estimation and size validation
- Full unwrap path (recovering inner instructions from a wrapped tx)
- Address Lookup Table support on unwrap
