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

The `native` module and its `dex-swaps` dependency are compiled only for
non-wasm32 targets. Linked into the plain `wasm/rust-v1` package binary, they
would add wasm-bindgen imports and a second `map_events` export. The package
WASM keeps the base `env` imports and exports.

The [immutable v0.5.1 oracle](tests/fixtures/legacy-v0.5.1/README.md) runs the
actual released WASM graph. It proves:

- formatter parity: 27 synthetic event cases give the same rows as the v0.5.1
  `db_out`;
- one complete 2025 block: all 231 rows of mainnet block 350000026
  (2025-06-29) match, and an empty block gives none;
- six single-transaction blocks with asserted outcomes: an unmodified mainnet
  swap matches, and five cases are the accepted differences below;
- one current block (slot 461335700, 2026-10-25) in which 132 of 314
  transactions are v1 messages: native keeps all 29 v0.5.1 rows, in order
  within each transaction, and adds 32, each one of the corrections below. v1
  transactions have rows on both sides (8 in v0.5.1, 10 natively).

It does not prove parity on current blocks: deployed v0.5.1 predates the
extraction changes below, and the current block is re-encoded from public
JSON-RPC, not provider bytes. It is a bounded offline baseline, not complete
protocol or network coverage, and it does not resolve the extraction gaps listed
in the [DEX projector documentation](../dex-swaps/README.md).

### Differences from deployed v0.5.1

Production and stage still serve svm-dex v0.5.1. Native rows follow the current
extraction, which corrects the v0.5.1 behaviors below. Each correction keeps
real swaps or fixes misattributed ones, so it is kept as an accepted difference
(owner decision, 2026-10-25). Announce them with the native module identity.
The svm-dex v0.5.3 WASM package ships every correction except the last row:
its `dex_swaps` input is still `blocks_without_votes`.

| Change | v0.5.1 | Native | Corpus case |
| --- | --- | --- | --- |
| Raydium CLMM `SwapEvent` trailing bytes (v0.5.2: idls #145, #215) | No CLMM rows after the 2026-05-18 on-chain payload growth | Rows restored | Current block: 1 row (`native_restores_raydium_clmm_leg`) |
| Raydium CLMM `SwapRouterBaseIn` per hop (v0.5.2: #219) | Multi-hop router invocations are not split per hop | One row per hop, pool from each event's `pool_state` | None |
| Meteora DAAM `EvtSwap2` (v0.5.2: idls #146, #217) | No DAAM rows after the 2025-12-26 cp-amm upgrade | Rows restored | `rpc_2026_damm_v2_swap2`, with the `swap2` fix; current block: 11 rows (`native_restores_damm_v2_leg`) |
| Darklake realised fill (v0.5.2: #216) | Rows from the `Swap` intent event | Rows from the `Settle` event | None |
| Meteora DAAM/DLMM pending-swap queue (v0.5.2: #218) | Can drop the first row of a multi-swap transaction | Kept | None |
| Orca event/pool matching (#224, landed as #227) | After a rejected instruction, events shift onto later pools and the last hop is dropped | Each hop keeps its own pool, mints and amounts | `rpc_2026_orca_multihop` |
| PumpSwap tracking flag (#223) | `buy` with the volume-tracking flag yields no row | Row | `rpc_2026_pumpswap_tracking_flag`; current block: 19 rows |
| Meteora AMM with a temporary token account (#223) | No row when the destination is created and closed in the transaction | Mint inferred from `InitializeAccount*`; row | `rpc_2026_meteora_amm_temporary_account` |
| Meteora DAMM v2 `swap2` mode byte (#223) | No row | Row | `rpc_2026_damm_v2_swap2` |
| Pump.fun bonding curve: current `buy` arguments and `buy_exact_sol_in` (#223) | No row | Row | Current block: 1 row (`native_restores_pumpfun_bonding_curve_leg`) |
| Vote-key pre-filter | Drops any transaction with a static `Vote111…` key, which any sender can add | Kept | `mainnet_swap_transaction_with_vote_key` |

### Solana v1 messages

About 40% of mainnet transactions have been v1 messages since October 2026:
the compute budget is carried inline, and there are no address table lookups.
`dex_swaps::project` accepts them; later versions and a v1 message with lookups
still fail closed. On the current block above, native rows equal the current
wrapper's (`dex_swaps::map_events` then `svm_dex::changes`), keys and ordinals
included.

The `svm-dex` WASM package never read the message version: neither the
`dex-swaps` v0.5.1 and v0.5.2 inputs nor the v0.5.3 `map_events` wrapper checks
it. A release cut from main before v1 support would therefore not have
regressed on v1 transactions. Only native consumers did: before v1 support,
`dex_swaps::project` rejected every block that held a v1 message.

### Committed package tests

`cargo test -p svm-dex committed_` runs the WASM embedded in the committed
`spkg/svm-dex-v0.5.3.spkg` (`blocks_without_votes`, then
`dex_swaps:map_events`, then `db_out`, as wired in the package) with a bounded
test-only interpreter. It must equal `native::changes` on the five pinned
complete blocks and on the single-transaction cases of the v0.5.1 corpus,
except the Vote-key case above, and its `db_out` must equal
`svm_dex::changes` on the 27 synthetic event cases.
`spkg/svm-clickhouse-dex-v0.5.3.spkg` must embed the same graph, every package
these manifests build or import must exist under `spkg/` with matching name and
version, and a released file must keep its recorded SHA-256. A decoder or
formatter change therefore fails these tests until the manifest version is
bumped and the package is repacked under the new name. See
[releasing](../docs/releasing.md).

```sh
cargo test --locked -p svm-dex
cargo build --locked -p svm-dex --release --target wasm32-unknown-unknown
```

The interpreter and frozen historical binaries are test-only dependencies.
Native row formatting uses the workspace SDK/DatabaseChanges types, without
calling a WASM module.
