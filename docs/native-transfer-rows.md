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
formatting, so it is not a peak allocation budget. The `native` module is
compiled only for non-wasm32 targets: its projector dependencies would link
wasm-bindgen imports into the plain `wasm/rust-v1` package binary, which
exports only `db_out`.

The formatter preserves:

- SPL original transaction/outer/CPI positions, all supported classic Token and
  Token-2022 operations, zero/self transfers, and the mint-as-source / burn-as-
  destination convention;
- empty unknown mint/decimals, exact decimal integer strings, ordered authority
  accounts, signatures, fees and compute units;
- dense system transaction/instruction indexes, including balance-only
  transactions and non-row create-account instructions that consume indexes.
  As in the vote-filtered legacy system input, a transaction with a static
  `Vote111…` account key takes no slot unless it carries a system instruction;
- existing separate `system_transfer`, `system_transfer_with_seed` and
  `system_withdraw_nonce_account` tables, the self-transfer exclusion on
  `system_transfer`, and the conditional `blocks` row;
- SPL rows before system rows, with row ordinals retained.

This is the current wrapper contract, with one accepted difference. The
wrapper's system input, like both v0.3.0 inputs, is `blocks_without_votes`,
which drops every transaction whose static account keys include the Vote
program; any sender can add that key to a transfer. Native keeps the system
transfers of such a successful transaction, which then takes a slot (SPL rows
already match the wrapper, whose SPL input is unfiltered). These are real
transfers the legacy path loses, so they are kept and pinned rather than
mirrored. The contract does not imply persisted CPI success, balance-delta
equivalence, or parity with an older released SPKG. Consumers must apply their
configured table allowlist and qualify historical package output separately.
The host formatter does not silently widen that allowlist.

Primary keys are not part of the WebSocket contract, but SQL sinks and mixed
WASM/native backfills use them. `system_transfer` keys equal v0.3.0's and the
current wrapper's, except later slots in a block with the accepted difference.
`spl_transfer` keys are the v0.4.0 original positions, so they differ from
v0.3.0's dense three-column keys.

Validation: `cargo test --locked -p svm-transfers -p native-token
-p spl-transfers -p spl-token-common`. Tests compare the complete SPL row fields,
keys and ordinals with the current wrapper on synthetic edge cases and pinned
mainnet block 350000026. System cases cover dense indexes, Vote-key slots,
skipped failures, balance-only transactions, create/self/zero behavior, v0
loaded keys, CPI metadata and full `u64` amounts. The existing `native-token`
tests independently compare system instructions with the wrapper on that
mainnet fixture and all five supported variants. Database field order is not
contractual; row order is.

The [published v0.3.0 boundary corpus](../svm-transfers/tests/fixtures/legacy-v0.3.0/README.md)
executes the exact historical WASM pipeline: 17 synthetic cases match retained
row fields, six retain explicit accepted-output differences (including the two
Vote-key cases), and two reject source shapes the old package accepts or skips.
On the four pinned mainnet blocks, all 1,259 system rows match in fields, order
and keys, and every SPL difference is classified as zero, self, unchecked
mapping or unchecked with source = authority, with none unexplained. These
differences must be announced before a migration.
