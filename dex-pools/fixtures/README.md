# Captured mainnet pool creations

These fixtures contain **one selected complete transaction**, with its original
block metadata, encoded as `sf.solana.type.v1.Block`. They are **not complete
blocks** and are **not bytes returned by a Firehose provider**. Transaction
instruction order, inner instructions, account keys, loaded addresses, balances,
and runtime logs are preserved from public Solana JSON-RPC. The selected
transaction is at local index **0**; the original source index is recorded below.

All four full source blocks were fetched on **2026-10-26** from the public
`https://api.mainnet-beta.solana.com` endpoint with:

```json
{"jsonrpc":"2.0","id":1,"method":"getBlock","params":["SLOT",{"encoding":"json","transactionDetails":"full","maxSupportedTransactionVersion":1,"commitment":"finalized"}]}
```

Replace `"SLOT"` with the integer slot. Rewards use the RPC default. No
credentials were used. Full responses are not committed; their selected,
canonical JSON responses are committed beside the encoded fixtures.

| Prefix | Slot | Original index / source transaction count | Evidence |
| --- | ---: | --- | --- |
| `pump-v2-349999876-selected` | 349999876 | 1546 / 1688 | Historical 58-byte `CreatePool` instruction at 2.22; 341-byte event self-CPI at 2.34; truncated runtime trace, so projection rejects it |
| `pump-current-461766718-selected` | 461766718 | 62 / 365 | Current 70-byte `CreatePool` instruction at 2.23; 352-byte event self-CPI at 2.35; Token-2022 for both tokens and a non-SOL quote mint |
| `orca-v2-461667419-selected` | 461667419 | 287 / 349 | `InitializePoolV2` at outer instruction 2; `PoolInitialized` at runtime log index 26 |
| `aldrin-v2-193097357-selected` | 193097357 | 143 / 2479 | `Initialize` at outer instruction 2; underlying mint identity and decimals from its exact vaults' token balances |

All four transactions succeeded. The current Pump event is proved to be an
immediate self-CPI of its actual pool creation invocation. The Orca event is
proved to have been emitted by the actual initialization invocation. For these
two fixtures, tests require the complete runtime trace to align and assert the
expected pool, protocol token order, mints, vaults, decimals, token programs and
exact instruction/event provenance.

The Aldrin fixture proves a successful initialization without a creation event.
Its base and quote mint identities come from transaction token balances for the
initializer's exact base and quote vaults. The test checks that protocol order,
both vaults, SPL token program, decimals (including zero), and instruction position
are preserved; the LP mint and fee accounts do not supply underlying identity.

The historical Pump fixture has a `Log truncated` marker. Its initialization and
event bytes still decode into a matching V2 candidate, but the missing runtime
evidence means the complete projection **must emit zero pools**. The test checks
both compatibility of the raw candidate and rejection by the evidence gate.
No logs are removed or fabricated to make a captured transaction pass.

## Source identities

| Prefix | Transaction signature | Block hash |
| --- | --- | --- |
| `pump-v2-349999876-selected` | `5rg7xHWAG6jDohDMbHEboHw3z7tDpoXdiLWRMTQ9fcFEWKEMBUE9nuyJvssA6xA3ZfV8b1ppGPreC5UzXtBX5omX` | `7LSBviCXpWaX7mCCADWSYHUg3kyaoy4VkMgJHtTEme91` |
| `pump-current-461766718-selected` | `4kEbHbpER2yoxyjFz5Gf9UxQTckXEFMcbqZTcRbVEXCZhJssSYXCpZrqfBFCpeFoQgw77w36zox7iKf2YtzU6uBb` | `DPZrRgm6hiPVaNjfWXQ2yDs2ix453MEZ7BWCHswih8V2` |
| `orca-v2-461667419-selected` | `3TkKYXzYcfsw7Bpr6tmXgEjnstHdQY5jpxzYmWQ6YosMyjnomN5wy9pCAfdh87THpaPEziirhaS1D8X9yPSmsUoc` | `2CAfXeKWCj92Uf1WUdSVzWx7uiXdvVJCAvxXdXAguuZd` |
| `aldrin-v2-193097357-selected` | `4C1z3zxydcyVKGJFN8ahpwBGMfjmQGYHrMMoFzi768ue1eVYeGDbomcSSYVV4RdRWmKyb5bTyqZ4CK9ymjBxA1GJ` | `3toCBN286HkrdW2QDAJQXpapSwKhGKGsby37P1PFKsh1` |

The Orca transaction was located through the 64-tick fee-tier account
`HT55NVGVTjWmWLjV7BrSMPVZ7ppU8T2xE5nCAZ6YaGad`. The Pump transactions were
located through the withdrawal authority
`39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg`; candidate signatures were then
verified against the full block response, not treated as creation evidence on
their own.
The Aldrin transaction was located through pool
`931bAjQx4RtbBjreP8HTNEu7ykozk4QXyYj3hiaJNdH`; its transaction response was then
verified and selected from the full block response without changing its contents.

## Conversion and limitations

[`select_and_convert.py`](select_and_convert.py) selects a transaction without
changing its contents and reuses the repository's
[`rpc_to_pb.py`](../../spl/transfers/tests/fixtures/rpc/rpc_to_pb.py). That
converter follows the Firehose poller field mapping; its
[documented limitations](../../spl/transfers/tests/fixtures/rpc/README.md#conversion)
still apply. In particular, it does not reproduce the bincode bytes of failed
transaction errors (none of these selected transactions failed). No provider
capture was compared to these outputs. No account snapshot or RPC enrichment is
used by the pool decoder.

To regenerate from a saved full response:

```sh
python3 -I dex-pools/fixtures/select_and_convert.py getBlock-349999876.json 349999876 1546 /tmp/pump-v2
```

To reproduce from the committed selected JSON, decompress it and select index
zero. Both output files must match byte for byte:

```sh
gzip -dc dex-pools/fixtures/pump-v2-349999876-selected.json.gz > /tmp/pump-selected.json
python3 -I dex-pools/fixtures/select_and_convert.py /tmp/pump-selected.json 349999876 0 /tmp/pump-reproduced
cmp /tmp/pump-reproduced.pb.gz dex-pools/fixtures/pump-v2-349999876-selected.pb.gz
cmp /tmp/pump-reproduced.json.gz dex-pools/fixtures/pump-v2-349999876-selected.json.gz
```

The script refuses to overwrite files and performs no network requests.

## SHA-256 evidence

| Prefix | Full RPC response bytes | Full RPC response SHA-256 |
| --- | ---: | --- |
| `pump-v2-349999876-selected` | 7044984 | `26f9c304be465c8a09047a568bee6e1812744e415ce94adabb7cd49c1c167709` |
| `pump-current-461766718-selected` | 2431006 | `1a4dca6fcbeb03c2395d37968f06f3cccc81d2c7eb5248e212bafb1bae5cd484` |
| `orca-v2-461667419-selected` | 1973417 | `dde5ab08e529f32d2e57950b037b1cf2f408c72482d0e4f83ea057e0caae80b4` |
| `aldrin-v2-193097357-selected` | 4521337 | `315efe0d8dd91d963966b79b7cdd00225acb469a8d534f245124532d542a2547` |

| Prefix | Encoded protobuf bytes | Encoded protobuf SHA-256 |
| --- | ---: | --- |
| `pump-v2-349999876-selected` | 15348 | `3aecbc2513e47ac36a5ae394e7e496592cafd5229d777fcf5ad9773a0fc9e622` |
| `pump-current-461766718-selected` | 18698 | `c75669f6f39b0dffb3cae60f18d8d47974ac08ef6a816075844fed68c54e885f` |
| `orca-v2-461667419-selected` | 4877 | `dbd820bc1d8b17e15db38b02c642d38827018b5458c09326cdcf80ab800793ed` |
| `aldrin-v2-193097357-selected` | 3476 | `74be9ca3a061c137f8dd90374198a93d2bd522da97f2fec36a5717d8396facb6` |

| File | SHA-256 |
| --- | --- |
| `pump-v2-349999876-selected.pb.gz` | `54c0c56d115d33e809e11073bf22b1e9837a37a9e3b6014aa8516340edbfb191` |
| `pump-current-461766718-selected.pb.gz` | `0f23d56c9917deffbde970f641bb2430835e73f976667f11f3ad375e533f2ddf` |
| `orca-v2-461667419-selected.pb.gz` | `7aa590d930eae876c0658a2ad99286c70def9ac7d690f988298e865a68e48e46` |
| `pump-v2-349999876-selected.json.gz` | `9b4f11173f72b132165a149f5ba32acab21a4914cc47165be2a76b3b65657cfd` |
| `pump-current-461766718-selected.json.gz` | `b7a4aae090a2ef5fbec98a367c43fdb64ce7a1e1ba24adbe1d5d530cc926a56b` |
| `orca-v2-461667419-selected.json.gz` | `8d5ff24f2fa3b9b847e043f68a746702e45e808461a14b0659f67b0c0ad23836` |
| `aldrin-v2-193097357-selected.pb.gz` | `895ae10db0daefdddada519593c12148682a778e0b5515fcabb8e6707921e31b` |
| `aldrin-v2-193097357-selected.json.gz` | `44f2ae60f25b65cddbabdc9990ebea97958aacfbc9d7ca00b2022c5ae6aa4c79` |

These four transactions demonstrate the listed layouts only; they do not prove
all historical initialization variants, every mint extension, full-block pool
coverage, backfill completeness, or a deployed hosted-store round trip.
