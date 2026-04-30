# squads-sdk

Rust SDK for wrapping and unwrapping Solana transactions in the Squads V5 multisig format (`executeTransactionSyncV2`).

## Features

- **Wrap** — take swap instructions and wrap them into a Squads multisig transaction, with optional ALT support
- **Unwrap** — recover inner instructions from a wrapped transaction (with or without ALTs)
- **Settings parsing** — parse on-chain Squads V5 settings accounts (members, threshold, etc.)
- **PDA derivation** — derive settings and vault PDAs
- **Preflight validation** — check CPI account limits and estimated tx size before wrapping
- **Detection** — identify whether a transaction is Squads-wrapped

## Error variants

| Variant | When |
|---------|------|
| `InvalidConfig` | Empty members, zero threshold, threshold > members |
| `CpiAccountLimitExceeded` | Inner instructions + Squads overhead > 64 accounts |
| `TransactionSizeExceeded` | Wrapped tx exceeds size limit |
| `UnrecognizedDiscriminator` | Transaction doesn't contain a Squads V2 instruction |
| `InvalidBase64` / `InvalidTransaction` | Malformed input |
| `InvalidSettingsData` | Settings account data is corrupted or truncated |

## Limitations

- Only supports `executeTransactionSyncV2` (not V1)
- Unwrap requires the caller to provide resolved account keys for ALT transactions (no RPC)
- Wrap produces unsigned transactions — caller must collect member signatures
- Squads CPI is limited to 64 accounts
