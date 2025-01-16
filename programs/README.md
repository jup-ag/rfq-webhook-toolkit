# Order engine program

## Build

`cargo build-sbf`

## Test

Native with stubbed VM

`cargo test`

With sbpf VM

`cargo test-sbf`

## Verifiable build

`solana-verify build --library-name order_engine -- --features production`
