# `substreams-svm` Repository Navigation Guide

This guide is a quick map for contributors and coding agents working in this monorepo.

## 1) What this repository is

`substreams-svm` is a Rust workspace of Solana/SVM Substreams packages for:

- protocol-specific decoders (DEXes, SPL/native, NFTs, staking)
- aggregate `DatabaseChanges` modules (`svm-*` crates)
- sink-facing packages for ClickHouse (`svm-*/clickhouse`)

## 2) Top-level layout

- `Cargo.toml`: workspace members and shared dependency versions.
- `proto/`: shared protobuf definitions in `proto/v1/`, pinned Buffa generation,
  and committed owned/view/lazy-view Rust models. See [protobuf development](protobuf.md).
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

The reusable scripts resolve paths relative to their own repository and refuse
to replace an existing crate, source file or schema:

- `scripts/gen-dex.sh <name> <description> <program_id> <shape>` creates a crate
  and manifest scaffold using workspace dependencies. The shape is a planning
  hint (`swap-only`, `swap-xy`, or `buy-sell-events`), not an implemented decoder.
  Review the program ID, workspace membership and local
  `spkg/solana-common-v0.4.0.spkg` dependency before building or running it.
- `scripts/gen-swap-proto.sh <name> [package]` creates a swap-only schema and
  runs `make -C proto protogen` with the pinned Buffa plugin. It never handwrites
  generated Rust. The default package replaces hyphens in the name with
  underscores; an explicit package must also match the manifest's output type.
- `scripts/gen-swap-lib.sh <crate> <idl_module> <instruction_enum> <swap_variant>
  <proto_path>` creates the Buffa-aware transaction wrapper. Implement the
  protocol-specific `process_instruction` and validate its field meanings before
  treating the scaffold as a decoder. The enum and variant arguments describe
  that remaining work; the script does not infer their semantics.

For an existing module, edit its canonical source/schema deliberately and follow
[protobuf development](protobuf.md); the generators have no overwrite option.
`scripts/gen-batch2.sh` is retired and exits without writes. Its historical
one-off schema rewrites and hand-written Prost models must not be reused.

### Add or edit aggregate database mapping logic

- Domain crates: `svm-balances`, `svm-transfers`, `svm-accounts`, `svm-metadata`, `svm-native`, `svm-spl`, `svm-nfts`, `svm-staking`, `svm-dex`.
- Entry point is typically `src/lib.rs` plus per-protocol files in `src/*.rs`.

### Native (non-WASM) projectors

- `spl/transfers` (`spl-transfers`): storage-independent SPL transfer projection over complete blocks, callable from native Rust (Pinax Streams, Firehose Parquet). It has no protobuf or Substreams dependency: use `block_view_buffa!` for the current Solana 0.16 model, or the unchanged `block_view!` for Prost models. `block_view_buffa!` fails closed: it rejects a block containing an unsupported message version or inline configuration. `block_view_buffa_lenient!` instead reports those transactions so production Substreams modules can skip them without halting; other transactions keep their original indexes. Fixtures, coverage and validation rules are in its README.
- `spl/transfers-substreams`: the `spl_transfers` Substreams package, a thin SDK 0.8/Buffa wrapper over `spl_transfers::project` (handler exported only on wasm32). Its output types come from the shared `proto` crate (`solana.spl.transfers.v1`).
- `spl/token-common` uses the same `spl_transfers::decode` for amount-moving instructions, so `spl_token`, `svm-transfers` and the native projector agree. It uses the lenient adapter: unsupported message versions are logged and skipped rather than halting the stream.

### Native DEX projection

`dex-swaps::project(&Block)` validates complete blocks and adds original
transaction/event-instruction or log provenance, aggregator classification and,
from an exact alignment of the instruction walk with the invocation logs, each
row's evidence writer (`invocation`, its outcome and `aggregator_route`), a
`verified_context` where the row can be rebuilt from its own instruction, and an
`execution_ordinal`, while sharing the existing protocol extraction.
`dex-swaps/README.md` documents these fields, their limits and the count-once
rule; `dex-swaps/fixtures/native-projection.md` records the unchanged-output
baseline, and `dex-swaps/fixtures/native-execution-coverage.txt` the
per-protocol evidence counts on the pinned blocks.

`svm-dex::native::changes` consumes that complete projection after typed consumers
have borrowed it and formats the unchanged legacy `swaps` rows. It validates
clock, count and encoded-size bounds without another block decode or extraction.
See `svm-dex/README.md` and its immutable v0.5.1 WASM corpus for the qualified
offline comparison scope.

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
- `spl/token-common` (shared SPL Token parsing), `spl/transfers` (native projector), `spl/transfers-substreams`
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

- Generate all shared protobuf models: `make -C proto protogen`
- Build workspace WASM: `cargo build --target wasm32-unknown-unknown --release`
- Build/pack per package: `make build`, `make pack` (where Makefile exists)
- Local stream checks: `make noop` / `make gui` in module directories

## 7) Notes discovered during repo scan

- Many historical docs still mention old `db-svm-*` folder names; current structure is `svm-*`.
- Root `README.md` previously under-described current module coverage.
- `substreams run` fails on manifests importing `substreams-sink-sql-protodefs-v1.0.7.spkg` with substreams CLI 1.25.0 (`deprecated.proto` name conflict over `sf.substreams.sink.sql.v1.Service`); dropping that import from a local copy works around it for verification.
- Generator output is a scaffold, not protocol coverage evidence; review its
  schema, decoding logic, manifest imports and fixtures before release.
