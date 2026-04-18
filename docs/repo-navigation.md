# `substreams-svm` Repository Navigation Guide

This guide is a quick map for contributors and coding agents working in this monorepo.

## 1) What this repository is

`substreams-svm` is a Rust workspace of Solana/SVM Substreams packages for:

- protocol-specific decoders (DEXes, SPL/native, NFTs, staking)
- aggregate `DatabaseChanges` modules (`svm-*` crates)
- sink-facing packages for ClickHouse (`svm-*/clickhouse`)

## 2) Top-level layout

- `Cargo.toml`: workspace members and shared dependency versions.
- `proto/`: shared protobuf definitions in `proto/v1/*.proto`.
- `common/`: shared Rust helpers (`db.rs`, `solana.rs`).
- `dex/`: protocol-level DEX modules (one crate per DEX/program).
- `spl/`, `native/`, `nft/`, `staking/`: protocol-level non-DEX modules.
- `svm-*`: aggregate DB modules by data domain.
- `svm-*/clickhouse`: ClickHouse sink packages and schema DDL.
- `spkg/`: local SPKG dependencies used by aggregate manifests.
- `scripts/`: helper scripts for generating DEX boilerplate/proto glue.

## 3) Fast navigation by task

### Add or edit a DEX protocol decoder

1. Start in `dex/<protocol>/` and update `src/lib.rs`.
2. Confirm `substreams.yaml` block filter (`program:<PROGRAM_ID>`).
3. Ensure matching protobuf in `proto/v1/*.proto`.
4. Wire output into `svm-dex/substreams.yaml` imports + `db_out` inputs.
5. Update ClickHouse sink schemas in `svm-dex/clickhouse/`.

### Add or edit aggregate database mapping logic

- Domain crates: `svm-balances`, `svm-transfers`, `svm-accounts`, `svm-metadata`, `svm-native`, `svm-spl`, `svm-nfts`, `svm-staking`, `svm-dex`.
- Entry point is typically `src/lib.rs` plus per-protocol files in `src/*.rs`.

### Work on sink schemas

- ClickHouse: `svm-*/clickhouse/schema.*.sql` and `Makefile`.

## 4) Workspace module inventory

### Aggregate domain crates (`svm-*`)

- `svm-accounts`, `svm-balances`, `svm-transfers`
- `svm-metadata`, `svm-native`, `svm-spl`
- `svm-nfts`, `svm-staking`, `svm-dex`

### DEX crates (`dex/*`)

- Core/high-volume: Raydium (`amm-v4`, `clmm`, `cpmm`, `launchpad`), Jupiter (`v4`, `v6`), Pump.fun (`bonding_curve`, `amm`), Meteora (`amm`, `daam`, `dlmm`), Orca (`whirlpool`).
- Additional protocols: BonkSwap, Lifinity, Phoenix, OpenBook, PancakeSwap, Moonshot, Stabble, Darklake, DumpFun, GoonFi, Heaven, Plasma, Saros, Aldrin, Boop, ByReal, DFlow, Drift, Obric (`v2`, `v3`), OKX DEX, Sanctum, Serum, SolFi (`v1`, `v2`), PumpSwap.

### Non-DEX protocol crates

- `spl/token`, `spl/token-2022`, `spl/token-swap`, `spl/token-lending`
- `native/system`, `native/stake`, `native/vote`
- `nft/magiceden/m2`, `nft/magiceden/m3`, `nft/tensor`
- `staking/marinade`
- `metaplex`

## 5) Source-of-truth files

- Program ID filters: each module's `substreams.yaml` under `blockFilter.query.string`.
- Aggregate DEX wiring: `svm-dex/substreams.yaml`.
- SQL schema outputs: `svm-*/clickhouse/schema.*.sql`.
- Shared protobuf contracts: `proto/v1/*.proto`.
- Shared transform helpers: `common/src/solana.rs`, `common/src/db.rs`.

## 6) Build/test entry points

- Build workspace WASM: `cargo build --target wasm32-unknown-unknown --release`
- Build/pack per package: `make build`, `make pack` (where Makefile exists)
- Local stream checks: `make noop` / `make gui` in module directories

## 7) Notes discovered during repo scan

- Many historical docs still mention old `db-svm-*` folder names; current structure is `svm-*`.
- Root `README.md` previously under-described current module coverage.
- `scripts/gen-dex.sh` uses a hard-coded base path (`/data/workspace/substreams-svm`), so it may require adaptation before use in other environments.
