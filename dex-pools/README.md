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

The [coverage checklist](../docs/pool-decoder-coverage.md) lists every advertised
DEX package, implemented creation variants, source evidence and remaining gaps.
Coverage currently includes 25 protocol families; it does **not** yet satisfy
all advertised DEX protocols or every historical initialization layout.

Pump.fun AMM V2/current V3 and Orca's three initialization variants retain their
matched-event admission. Other covered protocols expose identity through shared
IDL initialization account decoders. Those records require the exact successful
initialization invocation and every successful ancestor; their evidence is
`Initialization`, and event-derived metadata is not invented. Order-book records
use their market account as identity. Bonding curves use their curve account.
Stabble retains every underlying mint in weight order, excluding its LP mint.

`project` validates the whole supplied block structurally and rejects unsupported
message shapes. It accepts the workspace's legacy, v0 and v1 admission policy.
Failed transactions produce no records. An exact alignment of instruction
positions with runtime logs proves invocation outcomes, including caught CPI
failures and ancestors. Missing, truncated, inconsistent or forged runtime
traces cannot establish a committed initialization.

SPL Token Swap, Saros and Aldrin initializers contain custody vaults rather than mint
addresses. Their identities use only token metadata for those exact vaults from
the supplied transaction. Conflicting pre/post mints, decimals or token programs
remain unknown and produce no record; swap direction, amounts or another user's
token account do not identify pool assets.

Malformed supported block structure, unsupported message versions, exceeded
limits and conflicting pool metadata return an error for the whole projection.
Unknown layouts and unproved creation evidence are omitted. Successful empty
projections still carry slot and block hash. Omissions mean unknown coverage,
not proof that no pools were created. Captured Pump, Orca and Aldrin fixtures
remain distinct from synthetic typed-initializer tests; synthetic coverage
is not proof of every live or historical deployment.

## Record meaning

- `address` is the actual pool, market or bonding-curve account, never an
  aggregator or program fallback.
- `tokens` retain complete protocol creation order, including multi-asset pools.
  They do not follow swap direction. Mints, token programs and optional vaults are matched to the
  initialization accounts; decimals come from matched events, typed initializer arguments, exact vault
  metadata, or the explicit native-SOL contract. Missing optional
  fields remain `None`; zero decimals is a present value, not absence.
- `is_native` distinguishes native SOL from SPL assets. Native SOL uses the
  normalized `So11111111111111111111111111111111111111112` asset ID, with no
  SPL token-program or custody-vault claim. Wrapped SOL uses the same mint bytes
  with `is_native = false`.
- Creation carries slot, block hash, signature, initialization position and
  either initialization-only evidence, an event instruction, or a log position.
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

The native exports borrow byte slices from any model, including Firehose
Parquet's Prost `Bytes` and Pinax Streams' Buffa models:

- `decode_pool_creation(program_id, instruction_data, resolved_accounts, event_data)`
  retains matched-event Pump/Orca decoding.
- `decode_pool_initialization(program_id, instruction_data, resolved_accounts)`
  decodes the other covered shared-IDL initialization layouts.
- `decode_pool_initialization_with_token_accounts(..., &[TokenAccountMetadata])`
  also resolves the exact custody vaults needed by SPL Token Swap, Saros and Aldrin.

All return an **unverified `Candidate`**. Matching fields or typed account
positions alone do not prove that the program accepted the payload or that its
ancestors committed. Context supplied by a native caller is also unverified.
Callers must apply equivalent execution admission, or use `project` with the
reexported core model. The raw boundary does not serialize an entire block.

A bounded tiny instruction view copies at most 32 account keys for matched-event
decoding or 255 for initialization decoding, plus 2048 instruction bytes, because
shared typed account helpers accept that view. Event input is limited to 2048
bytes. The contextual raw API accepts at most 512 token-account metadata records.
The larger initializer account bound accommodates current Pump quote-chain
accounts without truncating them. The independent `consumer-tests/` crate
exercises transport borrowing without enabling the Substreams runtime.

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
