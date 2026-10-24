# SVM DEX

This package combines supported Solana DEX protocol events into one database-ready stream.

## Included events

- Normalized swap, buy, sell, and fill style events across supported DEX protocols
- Shared transaction, block, and instruction context for downstream analytics
- One combined db_out output for database sinks and analytics pipelines

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.


## Native legacy rows

`svm_dex::changes(&Clock, &Events)` is the pure formatter also used by the
`db_out` WASM handler. For an already decoded complete block, call
`dex_swaps::project_with_limits` once, borrow its projection for typed consumers,
then move it into `svm_dex::native::changes(clock, projection, max_swaps, max_bytes)`.
The formatter enforces the exact encoded output budget after each candidate row,
before retaining it, so repeated transaction fields cannot expand across the
whole block before rejection. At most one candidate row and one transaction's
encoded signer list sit outside that budget; their size is bounded by the
validated input cap. Exceeding a limit returns an error without partial output.
The adapter checks clock identity, transaction order, row count and encoded input
and output sizes. Bounds are positive, at most 100,000 swaps and 64 MiB. These
bounds limit accepted data; they are not a process RSS guarantee. Input projection
limits and raw block admission remain the caller's responsibility.

The conversion moves fields instead of decoding or extracting a second time.
It preserves all legacy rows, including aggregator summaries and rows without
verified invocation context, in the original vector order. Do not filter or
reorder the projection before formatting. Typed execution admission and the
count-once rule remain separate; preserving a legacy row does not qualify it as
a typed swap. Unknown protocols keep their existing formatting behavior.

The [immutable v0.5.1 oracle](tests/fixtures/legacy-v0.5.1/README.md) compares
27 formatter cases and all 231 rows in the committed mainnet block 350000026
against the actual released WASM graph. It also tests an empty block. This is a
bounded offline compatibility baseline, not complete protocol or network
coverage. It does not resolve the extraction gaps listed in the
[DEX projector documentation](../dex-swaps/README.md).

```sh
cargo test --locked -p svm-dex
cargo build --locked -p svm-dex --release --target wasm32-unknown-unknown
```

The interpreter and frozen historical binaries are test-only dependencies.
Native row formatting uses the workspace SDK/DatabaseChanges types, without
calling a WASM module.
