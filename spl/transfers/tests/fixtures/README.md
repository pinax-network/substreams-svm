# Pinned Solana mainnet blocks

Complete `sf.solana.type.v1.Block` messages, gzip-compressed, fetched on
2026-10-11 with:

```bash
firecore tools firehose-single-block-client -o bytes --bytes-encoding base64 \
  solana.firehose.pinax.network:443 <slot> | base64 -d | gzip -9 -n
```

| File | Slot | Transactions | Used for | SHA-256 of the decompressed block |
|---|---|---|---|---|
| `solana-mainnet-349019252.pb.gz` | 349019252 (parent 349019247) | 1773 | Skipped slots 349019248–349019251, Token-2022 | `9bf53a0f660662c56686560b2f95516b084e781f2a271bfa471c396bb4e76f16` |
| `solana-mainnet-350000009.pb.gz` | 350000009 | 1451 | Top-level zero-amount `TransferChecked` (1366) | `73bcf87148a44bfc77bf50ba5bb051b2204f352438a709b8e2a8fdd375bf5463` |
| `solana-mainnet-350000010.pb.gz` | 350000010 | 760 | Failed transactions with top-level `TransferChecked` (22, 119) | `d6a8ba2ecd134cbef27efa65750c30e1938c829d0b45c2939a1943f0b2bb717e` |
| `solana-mainnet-350000026.pb.gz` | 350000026 | 1444 | Mint from a loaded address (776), temporary account (1282), extra accounts (1337), inner self transfer (1335) | `ec6e4f3c1805034babeee7e35498989f73224a8419903e164eaf0703fe760969` |

The pinned operations were checked against Solana JSON-RPC:

- transaction order with `getBlock` (`transactionDetails: "signatures"`);
- accounts, amounts and owners with `getTransaction` (`jsonParsed`);
- lookup-table key sources the same way.

Validation also passed on slots 350000000–350000040.

[`rpc/`](rpc/README.md) holds one current block, slot 461335700 (2026-10-25),
in which 132 of 314 transactions are v1 messages. It is Firehose-format but
re-encoded from public JSON-RPC, not fetched from a provider; its README
records the conversion, its checksums and its known approximation.
