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
- Both tables use `MergeTree`, so re-sinking a range appends rows instead of
  replacing them. Rebuild them rather than backfilling part of the history:
  apply the schema (it adds `inner_instruction_index` idempotently), truncate
  `spl_transfer` and `transfers`, then re-sink from the start block.
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
> writes 55 for them). The
> committed `spkg/spl-token-v0.4.0.spkg` this manifest imports was packed before
> v1 support; repack it from the current tree before releasing v0.4.0.
