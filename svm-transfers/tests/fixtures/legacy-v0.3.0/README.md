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
normal library dependencies.

`corpus.json` holds 23 complete synthetic blocks, their SHA-256 values, exact
oracle DatabaseChanges bytes and native/legacy row fields in emission order.
Only `spl_transfer` and `system_transfer` rows are selected. Field map order and
database primary keys are not compared; downstream WebSocket consumers do not
expose primary keys. This is not live provider qualification or full block-space
coverage. The current-wrapper tests separately check database keys and ordinals.

| Outcome | Cases | Meaning |
| --- | --- | --- |
| Equal row fields/order | 17 | Empty, system transfer/max/zero/self/CPI, SPL checked/max, mint/burn, Token-2022, multisig, failed transaction, vote filtering and balance-only transaction |
| Different accepted rows | 4 | Native SPL keeps zero/self rows omitted by v0.3.0; unchecked top-level/CPI transfers use the destination account and leave mint unknown, while v0.3.0 uses that account as mint and the authority as destination |
| Native source rejection | 2 | Malformed system instruction and unsupported message version; the old package skips or accepts them |

These six differences are **not waived parity failures**. Their exact outcomes
are frozen and explicitly asserted to prevent accidental changes being described
as compatibility. Any migration must resolve or announce the applicable decoder
changes and still verify its transport normalization and live source behavior.

To reproduce from a separately downloaded, unmodified release asset:

```sh
python3 svm-transfers/tests/fixtures/legacy-v0.3.0/extract_oracle.py /path/to/svm-transfers-v0.3.0.spkg
cargo test --locked -p svm-transfers published_v030_boundary_corpus
```

Only intentional corpus maintenance sets `UPDATE_SVM_LEGACY_CORPUS=1`. Review
the emitted differences and exact bytes before accepting a changed snapshot.
