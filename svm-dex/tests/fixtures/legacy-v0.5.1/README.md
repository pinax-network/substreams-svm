# Deployed svm-dex v0.5.1 oracle

The exact public [svm-dex-v0.5.1 release](https://github.com/pinax-network/substreams-svm/releases/tag/svm-dex-v0.5.1)
asset `svm-dex-v0.5.1.spkg` supplies this independent historical baseline. Do not
regenerate its WASM from current Rust source or substitute a similarly named
local package.

- Package: 2,097,201 bytes; SHA-256 `0774a2330e9d070d55f1e39d0b0c057415aaafec7df94602ac21671020c5bf46`.
- `db_out` module hash: `411d6a461ae98065f767bb84afbfa2dca028b295`.
- `dex_swaps:map_events` module hash: `bd388f2e39f5dcc237cfbdb8d6c96d9e5678c797`.
- `dex_swaps:solana_common:blocks_without_votes` module hash: `21d352415dfa0a192ed065d22b71fbe72881408b`.
- Frozen `corpus.json` SHA-256: `8d9524dcec73610d01fff588a4315d21e2ee94a77412e6f66b5d72597382f883`.

| Binary | Bytes | SHA-256 |
| --- | ---: | --- |
| `db_out.wasm` | 291933 | `c7e8e781484b615115c55761db49a165c25575a78a31b1bc85a3b978c674a5b4` |
| `dex_swaps.wasm` | 1229232 | `5f00c0e2dc5161fede0d8b25226a8c0ab8405823b4796e2b29692209aca4e087` |
| `blocks_without_votes.wasm` | 316182 | `3507399c9f8091276b72d750632c6979e78e42a0137de8cee1be5e61b8cdb392` |

`extract_oracle.py /path/to/svm-dex-v0.5.1.spkg` validates the exact package,
module entrypoints, binary indexes and hashes. It has no network access and
refuses to replace a differing existing fixture.

`src/native/tests/legacy_v051.rs` executes the original three-module graph:
complete block -> blocks without votes -> swap events -> database rows. There
is no program-index filter in this package. The test also invokes the historical
formatter on 27 synthetic cases (empty/default data, all protocol values,
unknown protocol, maximum integers/signers and dense row ordering).

The complete-block case uses the existing public
`dex-swaps/fixtures/solana-mainnet-350000026.pb.gz` (gzip SHA-256
`d2aae88318c0caa856a04d786073a740dc737c4bcee25f138f44282d5e04f597`). Its
protobuf re-encoding has SHA-256 `ec6e4f3c1805034babeee7e35498989f73224a8419903e164eaf0703fe760969`.
All 231 swap rows match, including aggregators, unverified contexts, fields,
ordinals and row order. The empty complete block produces no rows.
Database primary keys are not part of the WebSocket row contract and are excluded
from this historical comparison. Current-wrapper keys and ordinals are compared
separately. Field map serialization order is normalized; row order is not.

That block predates the extraction changes listed in
[Differences from deployed v0.5.1](../../../README.md#differences-from-deployed-v051),
so its parity says nothing about current blocks. `block_cases` pins those
differences instead.

## Single-transaction block cases

`block_cases` in `corpus.json` holds six Firehose-format blocks, each with one
real transaction. Each block runs through the same three-module v0.5.1 graph and
through `dex_swaps::project` and `native::changes`. The test asserts every
outcome below and freezes the block bytes, the clock, the oracle event and
DatabaseChanges bytes, and both row sets. The differences are accepted
corrections (owner decision, 2026-10-25), not waived parity failures.

| Case | Source | v0.5.1 rows | Native rows | Difference |
| --- | --- | --- | --- | --- |
| `mainnet_swap_transaction` | First swap transaction of block 350000026 | 1 `pumpfun_amm` | Same row | `none` |
| `mainnet_swap_transaction_with_vote_key` | The same, plus a read-only unsigned `Vote111…` key | 0 | The same row | `native_keeps_vote_key_transaction` |
| `rpc_2026_orca_multihop` | `aqua-orca-multihop.transaction.pb`, slot 454558265 | 2, with shifted pools and amounts | 3 correct hops | `native_orca_events_matched_to_pool` |
| `rpc_2026_meteora_amm_temporary_account` | `knet.transaction.pb`, slot 452949660 | 0 | 1 `meteora_amm` | `native_restores_meteora_amm_leg` |
| `rpc_2026_pumpswap_tracking_flag` | `oilinu.transaction.pb`, slot 452919823 | 0 | 1 `pumpfun_amm` | `native_restores_pumpswap_leg` |
| `rpc_2026_damm_v2_swap2` | `privacy.transaction.pb`, slot 453196517 | 1 `raydium_cpmm` | The same row, after a new `meteora_daam` row | `native_restores_damm_v2_leg` |

The vote-key case covers the `blocks_without_votes` pre-filter. That filter
drops any transaction with a static Vote program key, not only vote
transactions, and any sender can add the key to a swap. Native keeps such
transactions.

### Provenance of the 2026 cases

No Firehose block of a 2026 slot is committed. Fetching one would need provider
access, which this change does not use. The only 2026 inputs are the four
`dex-swaps/fixtures` transactions. They are public `getTransaction` (JSON)
results converted to `ConfirmedTransaction`. The conversion kept
`meta.loaded_writable_addresses` and `meta.loaded_readonly_addresses`, but not
`message.address_table_lookups`.

A Firehose block carries the lookups of every v0 transaction that loads
addresses; in block 350000026 all 1,444 transactions pass that count check, 161
of them with lookups. The native projector rejects the bare RPC form with
`LoadedAddressCount`. The real
lookup-table accounts and indexes are not in the fixtures and cannot be
recovered offline. These blocks are therefore Firehose-format, but not
provider-identical.

Each case wraps its transaction in a block at its real slot and UTC block time,
with placeholder block hashes. It restores one placeholder lookup: account
`[0xff; 32]`, with sequential indexes matching the transaction's real writable
and readonly counts. Nothing else in the transaction changes.

Neither pipeline reads lookup keys or indexes; both use the resolved address
lists. The test checks that the restored lookup is inert:

- v0.5.1 extracts byte-identical events with and without it;
- native rows equal the current wrapper's (`dex_swaps::map_events` then
  `svm_dex::changes`) on the unmodified transaction.

These cases are evidence about extraction on real 2026 instructions and logs.
They say nothing about Firehose source admission for those slots. Replace them
with provider-captured blocks once such blocks are pinned.

Only the [current block](#current-block-with-v1-messages) exercises Raydium
CLMM after its 2026-05-18 `SwapEvent` change: one row, classified by rule
rather than by its event bytes. `SwapRouterBaseIn` and the other v0.5.2 changes
without a case in the
[package README](../../../README.md#differences-from-deployed-v051) have no
fixture. Those differences rest on the v0.5.2 release validation, not on this
corpus.

## Current block with v1 messages

`released_v051_current_block_with_v1_messages` runs the same three-module graph
on [slot 461335700](../../../../spl/transfers/tests/fixtures/rpc/README.md)
(2026-10-25), in which 132 of 314 transactions are v1 messages. That block is
re-encoded from public JSON-RPC with the Firehose poller's mapping, not
provider bytes. v0.5.1 never reads the message version, so it decodes v1
transactions as it decodes v0 ones without lookups. Native used to reject the
whole block.

The test compares rows by transaction, because every added row shifts the
ordinals after it. In each transaction, native keeps every v0.5.1 row, in
order, and only adds rows. Each added row is classified, and any other
difference fails the test:

| Difference | Rows | Evidence checked |
| --- | ---: | --- |
| `none` | 29 | All v0.5.1 rows |
| `native_restores_pumpswap_leg` | 19 | In each successful transaction, one PumpSwap `buy`/`buy_exact_quote_in` with the `track_volume` flag per added row |
| `native_restores_damm_v2_leg` | 11 | v0.5.1 has no `meteora_daam` row in the block |
| `native_restores_raydium_clmm_leg` | 1 | v0.5.1 has no `raydium_clmm` row in the block |
| `native_restores_pumpfun_bonding_curve_leg` | 1 | One Pump.fun `buy` with the `track_volume` flag, or `buy_exact_sol_in`, per added row |

That is 29 v0.5.1 rows and 61 native rows. v1 transactions have rows on both
sides: 8 in v0.5.1 and 10 natively, the 2 extra being DAMM v2 legs. The block's
classes are the documented corrections in the
[package README](../../../README.md#differences-from-deployed-v051). This test
does not freeze its outputs in `corpus.json`; it reruns the pinned WASM.

The Wasmi 0.46 host is test-only, with 64 MiB memory, 4 MiB per input/output and
finite fuel (one billion for small maps; five billion when input exceeds 1 MiB).
It exposes only the required Substreams calls, traps unused JavaScript imports,
and has no stores, WASI, RPC, filesystem or network access. Fuel/host failures
fail the test; they are never treated as expected decoder differences.

Ordinary `cargo test --locked -p svm-dex` reruns historical WASM and verifies the
frozen corpus without changing it. For an intentional reviewed corpus change,
`UPDATE_SVM_DEX_V051_CORPUS=1 cargo test --locked -p svm-dex released_v051`
rewrites it only after comparison with the current native output succeeds.
Review the changed inputs, raw oracle outputs, normalized rows and hashes.

This corpus proves the stated cases only: formatter parity, one complete 2025
block, the six block cases and the classified current block. It does not qualify all protocol variants, live
source admission, reorg behavior, transport or deployment.
