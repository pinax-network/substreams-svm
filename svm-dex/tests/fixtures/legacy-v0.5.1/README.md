# Deployed svm-dex v0.5.1 oracle

The exact public [svm-dex-v0.5.1 release](https://github.com/pinax-network/substreams-svm/releases/tag/svm-dex-v0.5.1)
asset `svm-dex-v0.5.1.spkg` supplies this independent historical baseline. Do not
regenerate its WASM from current Rust source or substitute a similarly named
local package.

- Package: 2,097,201 bytes; SHA-256 `0774a2330e9d070d55f1e39d0b0c057415aaafec7df94602ac21671020c5bf46`.
- `db_out` module hash: `411d6a461ae98065f767bb84afbfa2dca028b295`.
- `dex_swaps:map_events` module hash: `bd388f2e39f5dcc237cfbdb8d6c96d9e5678c797`.
- `dex_swaps:solana_common:blocks_without_votes` module hash: `21d352415dfa0a192ed065d22b71fbe72881408b`.
- Frozen `corpus.json` SHA-256: `15ce78b8ddac914d9984478f4d110f4e641374734d860a04069cc306c5d4f47c`.

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

This corpus proves the stated cases only. It does not qualify all protocol
variants, live source admission, reorg behavior, transport or deployment.
