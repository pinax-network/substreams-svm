# Independent native consumer

This crate exercises the public `dex-pools` decoding API outside the main
workspace, with `default-features = false`. Its separate workspace prevents
other packages from silently enabling a Substreams runtime dependency. Prost is
used only to represent a downstream input model; package output remains Buffa.

Run from the repository root:

```sh
cargo test --manifest-path dex-pools/consumer-tests/Cargo.toml --locked
cargo tree --manifest-path dex-pools/consumer-tests/Cargo.toml --edges normal
```

The dependency tree must not contain the `substreams` runtime crate. The Solana
core model and the IDL decoder libraries are native libraries, not that runtime.

`tests/model_families.rs` shows the integration boundary for both consumers:

- **Pinax Streams:** borrow program, instruction-data and account slices from
  its already decoded Buffa `substreams-solana-core` model.
- **Firehose Parquet:** borrow the same slices from its Prost 0.14 model with
  `bytes::Bytes` fields. The test uses the exact relevant protobuf field shapes;
  it does not link or alter the downstream repository.

Resolve compiled account indices against static keys followed by loaded writable
and loaded readonly keys before calling `decode_pool_creation`. The fixture uses
only static keys. Supply the protocol event bytes, including its discriminator,
from the appropriate event instruction or decoded log. Preserve their original
transaction/instruction/log positions in the caller. No protobuf re-encoding or
conversion to a complete SDK block is required for this raw API.

For initialization-only protocols, call `decode_pool_initialization`. When the
initializer names custody accounts without their mints (SPL Token Swap, Saros
and Aldrin), use `decode_pool_initialization_with_token_accounts` and supply
metadata for those exact custody accounts. Conflicting metadata omits the
candidate. The returned token vector preserves every underlying asset in the
protocol's order, including pools with more than two assets.

The result is an **unverified `Candidate`**, not a committed pool observation.
Matching instruction and event fields alone does not prove that the event came
from that instruction, that the invocation and its ancestors succeeded, or that
the transaction committed. Native callers using the Solana core block model can
use `dex_pools::project` for the library's complete-block admission checks. A
consumer using another model must establish equivalent execution evidence before
persisting a candidate. This test does not claim full Firehose Parquet projection,
downstream integration, or live-source qualification.

The fixtures are synthetic Orca Whirlpool `InitializePoolV2` plus
`PoolInitialized`, and Pump.fun AMM `CreatePoolV2` plus `CreatePoolEventV2`,
plus the current Pump creation layout (`CreatePoolV3` / `CreatePoolEventV3`).
They are serialized with the pinned upstream IDL types. Both model families must
return the same nonempty identities; mismatched, mixed-version or truncated
events and invalid account indices must be rejected. Byte-slice identity checks
demonstrate that decoder inputs borrow from the original model. Captured current
mainnet fixture qualification is tested separately by the producer package;
these isolated tests establish the native export boundary.

Additional tests exercise Stabble's three-asset initialization and SPL Token
Swap with explicit vault metadata through both borrowed model families.
