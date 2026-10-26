# Native DEX pool creation

`dex-pools` decodes pool creation from an already decoded Solana block without
executing Substreams, reading a store, or making an RPC request. It shares the
workspace IDL parsers, structural validator and conservative invocation proof.
The separate [`dex-swaps-pools`](../dex-swaps-pools/README.md) package feeds a
legacy Substreams key-value store and a Hosted Store from one shared
`map_pool_creations` decode node. The native API is independent of either store.

```rust,ignore
use dex_pools::{project, Block};

fn pools_from(block: &Block) -> Result<(), dex_pools::ProjectError> {
    let projection = project(block)?;
    for pool in &projection.pools {
        // Pool identity and mint order are already decoded and validated.
        // Native callers may consume these records before the sink adapter.
        println!("{:?}: {:?}", pool.protocol, pool.address);
    }
    Ok(())
}
```

The public `Block` and `solana` exports use `substreams-solana-core` 0.16 and Buffa.
There is no `substreams` runtime dependency. Results are plain native structs,
without store, RPC, or output protobuf types. Block protobuf decoding and its
memory budget remain the caller's responsibility.

## Admission and coverage

| Protocol | Covered initialization | Required evidence |
| --- | --- | --- |
| Pump.fun AMM | Historical `CreatePoolV2` and current `CreatePoolV3` | Typed initialization/account layout and a matching same-version `CreatePoolEventV2/V3` self-CPI whose immediate parent is that initialization. |
| Orca Whirlpool | `InitializePool`, `InitializePoolV2`, `InitializePoolWithAdaptiveFee` | Typed initialization/account layout and matching `PoolInitialized` log written by that initialization. |

Pump.fun bonding curves and AMM V1 creation, all other protocols, historical
initializations without the required event, and unknown layouts are excluded.
The IDL exposes a V1 enum but its parser does not decode that variant; this crate
does not guess or add a local layout fallback. The current creation layout uses
the shared IDL V3 variant with its complete official suffix; new fields are
never silently truncated into V2. Initialization/event version pairs and their
shared mode/fee/reward fields must match. Synthetic tests cover the listed
layouts; captured-chain fixture coverage is recorded separately in `fixtures/`.
This package does not promise a complete historical registry.

`project` validates the whole supplied block structurally and rejects unsupported
message shapes. It accepts the workspace's legacy, v0 and v1 admission policy.
Failed transactions produce no records. A pool also requires an exact alignment
of instruction positions with runtime invocation logs, including successful
outcomes for its event, initialization and every ancestor. Caught CPI failures,
missing/truncated logs, forged runtime-shaped program messages, another program's
event, mismatched accounts and stale events cannot establish a pool.

Malformed supported block structure, unsupported message versions, exceeded
limits and conflicting pool metadata return an error for the whole projection.
Unproved or unknown creation evidence is omitted. Successful empty projections
still carry slot and block hash. Omissions mean unknown coverage, not proof that
no pools were created.

## Record meaning

- `address` is the actual pool account, never an aggregator or program fallback.
- `tokens` retain protocol order: Pump base/quote or Orca A/B. They do not follow
  swap direction. Mints, token programs and optional vaults are matched to the
  initialization accounts; decimals come from its matched event. Missing optional
  fields remain `None`; zero decimals is a present value, not absence.
- Creation carries slot, block hash, signature, initialization position and
  event instruction or original log position.
- `creation.transaction_index` indexes the **supplied block**. In a
  `blocks_without_votes` input the transaction array is already compacted.
  Use signature plus instruction/log evidence as identity across filtered and
  complete-block transports.
- Results sort by raw pool address. Identical observations in one block keep the
  first source observation. A second observation with different protocol, mints,
  vaults, decimals or token programs rejects the entire block. Cross-block
  conflicts and snapshot/replay policy are the sink's responsibility.

The wrapper's legacy `store_pools` and Hosted Store output `map_pool_entries`
apply independent first-write policies to the same per-block observations.
`map_pools` emits only the legacy store's new-key deltas; Hosted Store ingestion
does not consume those deltas or depend on that store. The native projector does
not track whether a pool appeared in an earlier block.

This is creation metadata, not live reserves, ownership, fees, price, mint
metadata, transaction balances or a token-account registry. Pool updates and
closures are not projected. Starting at a recent slot cannot recover older
inactive pools; deployments need an explicit historical start or a separately
qualified snapshot.

## Other native transport models

`decode_pool_creation(program_id, instruction_data, resolved_accounts, event_data)`
borrows byte slices from any model, including Firehose Parquet's Prost `Bytes`
and Pinax Streams' Buffa models. It calls the same typed IDL decoding as `project`,
with no whole-block serialization. A bounded small instruction view copies at
most 32 account keys and 2048 instruction bytes because the upstream typed
account helpers accept that view. Event input is limited to 2048 bytes.

Its result is deliberately named `Candidate`: matching event fields alone do
not prove that the event was written by the expected invocation or committed.
Callers must provide their own equivalent execution-evidence admission, or use
`project` with the reexported core model. This crate does not provide full
model-independent block projection; the raw decoder is the shared decoding
boundary. The isolated `consumer-tests/` crate demonstrates both model families
without a Substreams runtime and includes an explicit negative mismatch test.

## Bounds and checks

`project_with_limits` bounds transactions, instructions, log lines, log bytes,
input bytes, account/balance references and output pools before growing the
projection or invocation trace. The limits cover memory this decoder consumes;
they do not retroactively limit an already decoded block. All counts use
saturating arithmetic. The structural view borrows payloads and allocates only
bounded reference vectors. Unknown large creation payloads are omitted.

Run:

```sh
cargo test --locked -p dex-pools
cargo test --manifest-path dex-pools/consumer-tests/Cargo.toml
```

Tests cover all listed instruction layouts, exact source provenance, failed
transactions and caught ancestor failures, incomplete/forged logs, mismatched
and wrong-parent events, duplicate/conflicting records, unknown variants,
resource bounds, supplied-block index semantics and transport borrowing.
