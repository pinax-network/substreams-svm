# `svm-transfers`

`DatabaseChanges` for SPL token and native SOL transfers. The ClickHouse sink
package and schema live in [`clickhouse/`](clickhouse/).

## Upgrading to v0.4.0

- `spl_transfer` and `transfers` rows are keyed by **original block positions**:
  `transaction_index` and `instruction_index` are the transaction's and outer
  instruction's positions in the complete block, and the new
  `inner_instruction_index` is 0 for an outer instruction, else 1 + its
  position among the inner instructions. Earlier versions wrote dense indexes
  over filtered transactions and instructions, so the same columns mean
  something else for every existing row.
- Every module hash changes, so the sink cursor must be reset and the whole
  history re-sunk. The tables use `MergeTree`, so re-sinking a range appends
  rows instead of replacing them, and that includes `native_transfers`, which
  the three `mv_system_*` views fill and which has no TTL. Deploy v0.4.0 into a
  fresh database. To reuse a database instead, drop the materialized views and
  truncate every table (`spl_transfer`, `transfers`, `system_transfer`,
  `system_transfer_with_seed`, `system_withdraw_nonce_account`,
  `native_transfers` and `blocks`), apply the new schema, which recreates the
  views (every view is `CREATE ... IF NOT EXISTS`, so an existing
  `mv_spl_transfer` would otherwise keep its old definition), then reset the
  cursor and re-sink from the start block.
- `spl_token` v0.4.0 validates each complete block before decoding it. A
  structurally invalid block (for example a missing transaction meta, an
  account index out of range or an inconsistent inner instruction group) is an
  error that halts the stream, where v0.3.0 never returned one. Unsupported
  message shapes are still skipped, not failed.
- Unchecked `Transfer` rows now have an empty `mint` (the instruction does not
  name it) and the destination account in `destination`; zero amounts and self
  transfers are kept in `spl_transfer`. The `transfers` view still filters on
  `amount > 0 AND mint IS NOT NULL`, which does not exclude empty mints.
- `system_transfer` (native SOL) keeps its previous dense indexes.

## Solana v1 messages

About 40% of mainnet transactions have been v1 messages since October 2026:
the compute budget is carried inline, and there are no address table lookups.
Deployed v0.3.0 decodes them, because it never reads the message version. The
native formatter (`native::changes`, see
[native transfer rows](../docs/native-transfer-rows.md)) and the current
`spl_token` v0.4.0 input accept them as well; later versions and a v1 message
with lookups still fail closed. The
[v0.3.0 oracle](tests/fixtures/legacy-v0.3.0/README.md#current-block-with-v1-messages)
compares both pipelines on a current block with 132 v1 transactions.

> **Warning.** A release cut from main before v1 support would have regressed
> against deployed v0.3.0: its `spl_token` v0.4.0 input skipped v1
> transactions and so dropped their SPL rows, 59 of 296 on that block (v0.3.0
> writes 55 for them). The `spkg/spl-token-v0.4.0.spkg` that v0.4.0 imports is
> packed from a tree with v1 support, and the
> [committed package tests](#committed-package-tests) fail if it is stale.

## Committed package tests

`cargo test -p svm-transfers committed_` runs the WASM embedded in the
committed `spkg/` files with a bounded test-only interpreter
([`tests/support/package_wasm.rs`](tests/support/package_wasm.rs)). The module
graph, inputs and block filters are read from each package, not hard-coded.

- `spkg/svm-transfers-v0.4.0.spkg` (`db_out` over the packed `spl_token` and
  `native_token`) must equal `native::changes` on the five pinned complete
  blocks, rows, keys and ordinals included, and the
  [v0.3.0 boundary corpus](tests/fixtures/legacy-v0.3.0/README.md) must show
  only its documented differences.
- `spkg/svm-clickhouse-transfers-v0.4.0.spkg` must embed the same graph.
- `spkg/spl-transfers-v0.1.0.spkg` with params `all` must equal
  `spl_transfers::project` with `Coverage::ALL`.
- Every package these manifests build or import must exist under `spkg/` with
  matching name and version, and a released file must keep its recorded
  SHA-256.

A decoder or formatter change therefore fails these tests until the manifest
version is bumped and the package is repacked under the new name; a released
file is never repacked in place. See [releasing](../docs/releasing.md).
