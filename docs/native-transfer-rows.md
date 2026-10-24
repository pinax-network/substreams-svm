# Native transfer database rows

`svm-transfers::changes(clock, spl_events, system_events)` is the database
formatter used by the `db_out` WASM handler. It can also run in native Rust
without a Substreams host, store or network connection. The SDK is still a
dependency because the compatibility format uses its `Clock` and
`DatabaseChanges` types. Instruction extraction remains in `spl-transfers` and
`native-token`.

`svm-transfers::native::changes(clock, spl_projection, system_projection,
max_operations, max_output_bytes)` formats the already computed full projections
of one decoded block. It does not decode protobuf or extract instructions again.
The SPL projection must have `Coverage::ALL`; neither projection may be filtered
or reordered. The block identity, timestamp, combined operation count and
encoded event/output sizes are checked. Callers must bound protobuf allocations,
transactions and instructions before projection. Output size is checked after
formatting, so it is not a peak allocation budget.

The formatter preserves:

- SPL original transaction/outer/CPI positions, all supported classic Token and
  Token-2022 operations, zero/self transfers, and the mint-as-source / burn-as-
  destination convention;
- empty unknown mint/decimals, exact decimal integer strings, ordered authority
  accounts, signatures, fees and compute units;
- dense system transaction/instruction indexes, including balance-only
  transactions and non-row create-account instructions that consume indexes;
- existing separate `system_transfer`, `system_transfer_with_seed` and
  `system_withdraw_nonce_account` tables, the self-transfer exclusion on
  `system_transfer`, and the conditional `blocks` row;
- SPL rows before system rows, with row ordinals retained.

This is the current wrapper contract. It does not imply persisted CPI success,
balance-delta equivalence, or parity with an older released SPKG. Consumers must
apply their configured table allowlist and qualify historical package output
separately. The host formatter does not silently widen that allowlist.

Validation: `cargo test --locked -p svm-transfers -p native-token
-p spl-transfers -p spl-token-common`. Tests compare the complete SPL row fields,
keys and ordinals with the current wrapper on synthetic edge cases and pinned
mainnet block 350000026. System cases cover dense indexes, skipped failures,
balance-only transactions, create/self/zero behavior, v0 loaded keys, CPI metadata
and full `u64` amounts. The existing `native-token` tests independently compare
system instructions with the wrapper on that mainnet fixture and all five
supported variants. Database field order is not contractual; row order is.

The [published v0.3.0 boundary corpus](../svm-transfers/tests/fixtures/legacy-v0.3.0/README.md)
executes the exact historical WASM pipeline: 17 cases match retained row fields,
four retain explicit accepted-output differences, and two reject source shapes
the old package accepts or skips. Those differences remain migration gates.
