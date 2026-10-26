# Published Solana transfer package boundary corpus

This test executes the actual
[svm-transfers-v0.3.0 release asset](https://github.com/pinax-network/substreams-svm/releases/download/svm-transfers-v0.3.0/svm-transfers-v0.3.0.spkg).
The 4,501,174-byte package SHA-256 is
`bc04960b7b18223e124a3ad5c3384817df0f66003a1ea4b989b9a80047e5b05e`;
its `db_out` module hash is `9a8896562eb15ba5cc7b0c36a5cfe7c3af6ec043`.
The repository's similarly named `spkg/svm-transfers-v0.3.0.spkg` is a different
ClickHouse package (`08efb62b...`), so it is **not** the oracle.

`extract_oracle.py` verifies the package size/hash, five module declarations and
entrypoints, and the hashes of four WASM binaries before extracting them. The
vote-removal and program-index entrypoints share the same binary. Existing
different fixtures are never overwritten. No network or build runs in extraction.

The test-only Wasmi 0.46 host runs vote removal, program indexing, the exact
published SPL/system program filters, both map modules, and `db_out`. Every
invocation has a fresh 64 MiB memory limit, one billion fuel units and 4 MiB
input/output limits. No stores, WASI, network, filesystem or JavaScript execution
is available. Unused wasm-bindgen imports trap if invoked. Unexpected host,
fuel, allocation or linking failures fail the test; only a registered package
panic can become an explicit oracle panic outcome. No interpreter is added to
normal library dependencies. The workspace builds only the interpreter crates
optimized in the dev/test profile, so the mainnet blocks below take seconds.

`corpus.json` holds 29 complete synthetic blocks, their SHA-256 values, exact
oracle DatabaseChanges bytes and native/legacy row fields in emission order.
Only `spl_transfer` and `system_transfer` rows are selected. Field map order is
not compared, and the stored native bytes omit database primary keys, which
downstream WebSocket consumers do not expose. Where the rows agree, the test
also asserts that `system_transfer` keys equal the oracle's (see
[primary keys](#primary-keys)). This is not live provider qualification or full
block-space coverage. The current-wrapper tests separately check database keys
and ordinals.

| Outcome | Cases | Meaning |
| --- | --- | --- |
| Equal row fields/order | 19 | Empty, system transfer/max/zero/self/CPI, SPL checked/max, mint/burn, Token-2022, multisig, failed transaction, a pure vote transaction before transfers, a balance-only transaction, a v1 message (`v1_message`) and a `Vote111…`-key create-account before a transfer (`vote_key_create_account_then_transfer`, keys equal too) |
| Different accepted rows | 6 | Native SPL keeps zero/self rows omitted by v0.3.0; unchecked top-level/CPI transfers use the destination account and leave mint unknown, while v0.3.0 uses that account as mint and the authority as destination; native keeps both transfers of a successful transaction with a static `Vote111…` account key, with or without a vote instruction, while v0.3.0 emits nothing |
| Native source rejection | 4 | Malformed system instruction, and three message shapes native does not cover: version 1 without the `versioned` flag (`unsupported_message_version`), version 2 (`v2_message`) and a v1 message with an address table lookup (`v1_message_with_address_table_lookups`); the old package skips or accepts them |

These ten differences are **not waived parity failures**. Their exact outcomes
are frozen and explicitly asserted to prevent accidental changes being described
as compatibility. Any migration must resolve or announce the applicable decoder
changes and still verify its transport normalization and live source behavior.

The two `native_keeps_vote_key_transaction` cases (`vote_key_transfers`,
`vote_instruction_and_transfers`) are an accepted difference. v0.3.0 feeds both
map modules from `solana_common:blocks_without_votes`, which drops every
transaction whose static account keys include the Vote program, not only vote
transactions; any sender can add that key to a transfer. Those are real
transfers the legacy path loses, so native keeps them (owner decision,
2026-10-25: keep native corrections and pin them, mirror legacy only where
clients rely on it). The current wrapper's `spl_token` v0.4.0 input is the
unfiltered block, so its SPL rows already include them; its `native_token`
v0.3.0 input is still vote-filtered. No transfer in the four pinned mainnet
blocks below is in such a transaction.

## Primary keys

The WebSocket normalizer ignores primary keys, but SQL sinks and mixed
WASM/native backfills use them.

- `system_transfer` keys are the dense `(block_hash, transaction_index,
  instruction_index)` of v0.3.0 and the current wrapper, whose system input is
  vote-filtered. Native assigns slots the same way: a transaction with a static
  Vote key takes no slot unless it writes at least one system row. A Vote-key
  transaction whose system instructions write none, such as a funded
  create-account (real vote-account creations) or a self transfer, takes no
  slot either; `vote_key_create_account_then_transfer` pins this. Keys equal
  the oracle's on all 1,259 system rows of the 2025 blocks below, on the 104 of
  the current block, and on every equal corpus case. Only a block with the
  accepted difference above differs: there the Vote-key transaction takes a
  slot, so later slots in that block are one higher than v0.3.0's.
- `spl_transfer` keys follow the v0.4.0 contract: the original
  `(block_hash, transaction_index, instruction_index, inner_instruction_index)`
  positions (see the [upgrade notes](../../../README.md#upgrading-to-v040)).
  v0.3.0 wrote three dense columns, so no SPL key matches it.

## Pinned mainnet blocks

`published_v030_pinned_mainnet_blocks` runs the same oracle over the four
complete blocks already pinned in
[`spl/transfers/tests/fixtures`](../../../../spl/transfers/tests/fixtures/README.md).
Each is under the host's 4 MiB input limit, so no fixture is added. For every
block it asserts that `system_transfer` rows are equal field for field, in
order and with equal primary keys, and that each SPL row is equal or in one
named class:

| Slot | System rows | SPL equal | Zero | Self | Unchecked mapping | Unchecked, source = authority | Unexplained |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 349019252 | 477 | 489 | 1 | 0 | 177 | 1 | 0 |
| 350000009 | 174 | 337 | 2 | 0 | 129 | 1 | 0 |
| 350000010 | 242 | 317 | 0 | 0 | 86 | 4 | 0 |
| 350000026 | 366 | 499 | 0 | 1 | 100 | 2 | 0 |

Zero, self and unchecked mapping are the synthetic classes above. The last
named class combines two of them. For an unchecked `Transfer` whose source is
also its authority (`accounts[0] == accounts[2]`), v0.3.0 writes the authority
as the destination, sees source == destination and drops the row as a self
transfer. Native emits it with the real destination and an empty mint. Every
legacy row has a native counterpart.

## Current block with v1 messages

`published_v030_current_block_with_v1_messages` runs the same comparison on
[slot 461335700](../../../../spl/transfers/tests/fixtures/rpc/README.md)
(2026-10-25), in which 132 of 314 transactions are v1 messages. That block is
re-encoded from public JSON-RPC with the Firehose poller's mapping, not provider
bytes. v0.3.0 never reads the message version, so it decodes v1 transactions as
it decodes v0 ones without lookups.

| Rows | System | SPL equal | Zero | Self | Unchecked mapping | Unchecked, source = authority | Unexplained |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Whole block | 104 | 270 | 4 | 0 | 18 | 4 | 0 |
| v1 transactions only | 27 | 48 | 4 | 0 | 7 | 0 | 0 |

System rows are equal in fields, order and keys. v1 rows appear on both sides:
v0.3.0 writes 55 SPL and 27 system rows for v1 transactions, native 59 and 27.
The four extra native rows are zero-amount transfers.

> **Warning.** Before v1 support, native rejected every block that held a v1
> message, and the current `spl_token` v0.4.0 (`spl-token-common`) silently
> skipped those transactions. A legacy release cut from main between the Buffa
> migration (`b2e4976`, 2026-10-17) and v1 support would therefore have lost
> the SPL rows of about 40% of current transactions against deployed v0.3.0.
> The first pack of `spkg/spl-token-v0.4.0.spkg` (`1e3e052`) predated v1
> support. The committed one is packed from a tree with it, and
> `committed_svm_transfers_package_on_the_v030_boundary_corpus` runs the
> committed svm-transfers v0.4.0 package on this corpus.

To reproduce from a separately downloaded, unmodified release asset:

```sh
python3 svm-transfers/tests/fixtures/legacy-v0.3.0/extract_oracle.py /path/to/svm-transfers-v0.3.0.spkg
cargo test --locked -p svm-transfers published_v030
```

Only intentional corpus maintenance sets `UPDATE_SVM_LEGACY_CORPUS=1`. Review
the emitted differences and exact bytes before accepting a changed snapshot.
