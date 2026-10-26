# Solana pool identities: legacy and Hosted Stores

The `dex_swaps_pools` package provides both a legacy Substreams key-value store
and a **Substreams-feed Hosted Store** producer for verified Pump.fun AMM and
Orca Whirlpool pool identities. One shared map, `map_pool_creations`, reads
`solana_common:blocks_without_votes` and decodes through the native
[`dex-pools`](../dex-pools) library. It imports no protocol-specific Substreams
maps and performs no RPC or store reads.

```text
solana_common:blocks_without_votes
  └─ map_pool_creations                 dex.pools.v1.Pools
       ├─ store_pools                   legacy set_if_not_exists
       │    └─ map_pools                Create deltas as dex.pools.v1.Pools
       └─ map_pool_entries              Hosted Store v2 SinkEntries
```

Both branches consume the same decoded records. The Hosted Store branch runs
independently of the legacy store: it neither reads `store_pools` nor consumes
`map_pools`. Selecting only the Hosted Store output does not require the legacy
store branch.

## Choose an output

| Module | Use | Meaning |
| --- | --- | --- |
| `map_pool_creations` | Inspect verified creation records or build another sink. | Observations in this block, with identical within-block duplicates collapsed. No historical store lookup. |
| `store_pools` | Read pool metadata from another Substreams module. | Legacy key-value store keyed by base58 pool address; protobuf `dex.pools.v1.Pool`; `set_if_not_exists`. |
| `map_pools` | Stream pools newly inserted into the legacy store. | Only `Create` deltas from `store_pools`, as `dex.pools.v1.Pools`. |
| `map_pool_entries` | Feed a Hosted Store. | Raw 32-byte pool keys and `Any` values, with `if_not_exist: true`. The Hosted Store applies its own first-write policy. |

`map_pool_creations` does not deduplicate across blocks. The legacy store and
the Hosted Store each retain their own first observed value for a key over the
history they ingest. A later differing value does not replace that first value.
The shared decoder rejects conflicting identities within a block before either
branch receives output. It does not detect cross-block conflicts, and neither
store enriches or updates an existing identity.

## Shared value contract

The value schema is [`dex.pools.v1.Pool`](../proto/v1/dex-pools.proto); map outputs
use the `dex.pools.v1.Pools` container. Token order is protocol order: Pump.fun
base/quote and Orca A/B. It is never the direction of a swap. Missing optional
metadata stays absent; zero decimals is a valid known value.

Values include the initialization invocation, event evidence, slot, block hash
and transaction signature. `input_transaction_index` refers to the supplied
block array; after vote filtering it is not the original full-block transaction
index. Use the signature and instruction position for identity.

Only successfully committed, attributable initialization records are admitted.
Missing/truncated/inconsistent invocation evidence produces no pool record;
malformed supported block structures and capacity violations are errors. See the
[native decoder](../dex-pools/README.md) for exact supported variants and fixture
coverage.

This is a creation registry, not a snapshot of every existing pool or a record
of changing reserves, fees, liquidity or ownership. Historical completeness
depends on the selected start slot, available history, supported variants and
recorded invocation evidence. Vote-containing transactions removed by the source
are outside this package's input. Pool addresses must not be reused for different
identities within a store deployment; rebuild with a different key strategy if
that becomes part of the supported domain.

## Legacy store use

Import the package and read `store_pools` from a consuming Substreams module:

```yaml
imports:
  dex_swaps_pools: ../spkg/dex-swaps-pools-v0.1.0.spkg

modules:
  - name: map_enriched_swaps
    kind: map
    inputs:
      - map: solana_common:blocks_without_votes
      - store: dex_swaps_pools:store_pools
```

This input fragment assumes the consumer also imports `solana_common` and
declares its own output type. Read with `StoreGetProto<Pool>`, using the generated
`dex.pools.v1.Pool` type and the pool address encoded as base58 text. To consume
only records first inserted into the legacy store, select `map_pools` or import
`dex_swaps_pools:map_pools` as a map input. `map_pool_creations` is the appropriate
map when the consumer needs every admitted creation observation rather than
store insertion results.

Store presence alone does not prove a pool existed before a particular swap in
the same block. Check the stored creation's transaction and instruction evidence
against the consuming operation; do not infer execution order from the order of
pool records in the map.

## Hosted Store deployment

`map_pool_entries` emits
`sf.substreams.foundational_store.model.v2.SinkEntries`:

- Key: the raw 32-byte pool address, rather than its base58 text.
- Value: `google.protobuf.Any` with type URL
  `type.googleapis.com/dex.pools.v1.Pool`.
- `if_not_exist: true`: the Hosted Store retains its first value for each key.
- Empty results still emit `SinkEntries`, allowing ingestion to advance.

1. Publish the reviewed `dex-swaps-pools-v0.1.0.spkg` at a public HTTPS URL or
   the Substreams registry.
2. In The Graph Market, choose **Hosted Store**, then **Substreams feed**.
3. Select network `solana`, package `dex_swaps_pools`, output
   `map_pool_entries`, and value type `dex.pools.v1.Pool`.
4. Choose a start slot that covers the desired pool creation history; stop `0`
   follows head. Starting near head does not discover inactive older pools.
5. Record the deployment ID and verify ingestion progress before consuming it.

A downstream Substreams declares:

```yaml
inputs:
  - map: solana_common:blocks_without_votes
  - foundational-store: <deployment-id>@v0.1.0
```

Deduplicate raw pool keys, call `FoundationalStore::get` once per batch, check
response codes and `Any.type_url`, and decode `dex.pools.v1.Pool`. Consumers wait
for the store to reach their block. The producer must not read its own Hosted
Store, which would make ingestion depend on itself. Reads have block
granularity; same-block creation/use still requires instruction-order validation.

The hosted service resolves the deployment ID; its required `@version` suffix
does not isolate schemas. Use a new deployment for breaking changes or rebuilds.
This package does not create a hosted deployment or supply a deployment ID.

See the [official Hosted Store guide](https://docs.substreams.dev/how-to-guides/hosted-services/hosted-store-substreams-feed).

## Build and verify

```sh
cargo test --locked -p dex-pools -p dex-swaps-pools
make -C dex-swaps-pools pack
substreams info spkg/dex-swaps-pools-v0.1.0.spkg
```

For a release, follow [`docs/releasing.md`](../docs/releasing.md), build the
workspace with its pinned toolchain, then pack this manifest. Its WASM declares
`wasm/rust-v1+wasm-bindgen-shims` because the shared Solana IDL dependencies
require those shims. Decoding runs in `map_pool_creations`; the store and sink
adapters only encode and route its verified records.

## Native consumers

firehose-parquet and pinax-streams should depend on `dex-pools`, without this
wrapper, to avoid the Substreams runtime and storage concerns. The native crate's
`project`, `project_with_limits` and `decode_pool_creation` exports are unchanged
by the choice of store. See its README and isolated consumer tests for Buffa and
Prost `Bytes` examples. Existing downstream pipelines are not changed by this
package.
