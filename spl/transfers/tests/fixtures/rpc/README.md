# Current Solana block with v1 messages

`solana-mainnet-461335700.pb.gz` is a complete `sf.solana.type.v1.Block` for
mainnet slot 461335700, compressed with `gzip -9 -n`. It is Firehose-format but
**not** bytes served by a Firehose provider. It was re-encoded from a public
JSON-RPC `getBlock` response by [`rpc_to_pb.py`](rpc_to_pb.py), which follows the
field mapping of the Firehose RPC poller (`streamingfast/firehose-solana`
`cbf1571`, `block/fetcher/rpc.go`). The Pinax reader runs that poller
(`firesol fetch rpc`; the packaged build is firesol v1.5.0), which already
emits v1 messages.

The block is from after Alpenglow, which removed vote transactions from blocks
at slot 459,653,000 (2026-10-21). It holds no vote transaction, and 132 of its
314 transactions are v1 messages. Before v1 support, the strict native views
rejected the whole block on the first one.

| | |
| --- | --- |
| Slot | 461335700; parent 461335699; block height 439364381 |
| `blockTime` | 1792947950 (2026-10-25 17:05:50 UTC) |
| Block hash | `FhN95bEwrHfV66eu7k5KP9h2vUknvUVhrKm98ZKKajH3` |
| Transactions | 314: 82 legacy, 100 v0 (46 with address table lookups), 132 v1; 83 failed; none with a `Vote111…` key |
| v1 messages | All 132 carry `transaction_config` and no address table lookups; 75 succeeded |
| Request | Public `api.mainnet-beta.solana.com`, 2026-10-25 about 17:06 UTC: `getBlock(461335700, {"encoding": "json", "transactionDetails": "full", "maxSupportedTransactionVersion": 1})`, with rewards (the default). The tip was then slot 461335747. |
| JSON response | 1,967,254 bytes, SHA-256 `966a3b085544a4e14c2b8cc2c207357c1d36d3d98c3ef7c5ee536a1a628532c3` (not committed) |
| Encoded block | 1,251,383 bytes, SHA-256 `0b129ca42a483cbe92f738a114b7b38830c6950efc42368f9d82f80a0a5db8b6` |
| Gzip file | 409,290 bytes, SHA-256 `200ec29a5bd1fc5b87cdb0cc00109d955d19b72d06f349b4ccd757d29c74746e` |

## Conversion

`rpc_to_pb.py` maps each JSON field to the field the poller sets, writes fields in
field-number order as the Go encoder does, and omits proto3 defaults:

- message `versioned` for every non-legacy message, `version` as the wire version
  (0 or 1, unset for legacy), and `transaction_config` only when a v1 message
  sets at least one of its four values;
- static keys, recent blockhash, signatures and lookup keys decoded from base58;
  instruction data from base58; return data from base64, omitted when empty;
- inner-instruction `stack_height` only when the RPC records a non-zero height;
- token balances with `program_id` empty when it is the zero key, and
  `ui_token_amount` only when present;
- block and transaction rewards sorted by lamports, as `toPBReward` does.

Known approximations:

- **`meta.err` is approximated.** The poller stores the bincode-encoded
  `TransactionError`, which the JSON response does not carry. A present error is
  written as 12 zero bytes. Which transactions failed is exact; the error bytes
  are not. This repository only reads whether the error is present.
- Go's reward sort is not stable; ties keep the JSON order here. This block has
  one reward, so it makes no difference.
- The poller's previous-hash and parent-slot patch tables cover a few historical
  slots only, so they are not applied.

No provider-captured block for this slot has been compared, so the bytes may
also differ in ways the mapping does not show. Replace this fixture with a
provider capture once one is pinned.

To regenerate it from a saved response (standard library only, no network):

```sh
python3 -I rpc_to_pb.py getBlock-461335700.json 461335700 block.pb
gzip -9 -n < block.pb > solana-mainnet-461335700.pb.gz
```

The script prints both SHA-256 values, which must match the table. It refuses
to overwrite an existing output file.

## Checks and uses

- `spl/transfers/tests/buffa_model.rs` decodes it with the repository's
  Buffa model (`substreams-solana` 0.16) and re-encodes it byte for byte. The
  older Prost model decodes it too (`projection.rs`, `buffa_model.rs`).
- Native admission: `spl-transfers`, the typed `spl_transfers` package,
  `native-token` and `dex-swaps` accept the block, and `spl-token-common` now
  processes its v1 transactions instead of skipping them.
- The deployed-legacy oracles run on it:
  `published_v030_current_block_with_v1_messages` (svm-transfers v0.3.0) and
  `released_v051_current_block_with_v1_messages` (svm-dex v0.5.1). See their
  fixture READMEs for the asserted counts.

The file is 1.25 MB decoded, under the 4 MiB input limit of the test WASM host.
