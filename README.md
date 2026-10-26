# Solana `Substreams` (SVM)

Substreams packages for Solana/SVM data indexing across:

- native and SPL token domains
- DEX and AMM protocols
- NFT and staking protocols
- sink-ready `DatabaseChanges` outputs for ClickHouse

## Repository Map

For a practical contributor map, start here:

- [`docs/repo-navigation.md`](docs/repo-navigation.md)

## Workspace Structure

- `dex/`, `spl/`, `native/`, `nft/`, `staking/`, `metaplex/`: protocol-specific decoders
- `svm-*`: aggregate domain packages that emit `DatabaseChanges`
- `svm-*/clickhouse`: ClickHouse sink manifests + schema files
- `proto/`: shared protobuf schemas
- `spl/transfers/`: native, storage-independent SPL transfer projector (no Substreams runtime), wrapped by `spl/transfers-substreams`
- `common/`: shared Rust helpers
- `dex-pools/`: reusable native Pump.fun AMM and Orca pool-creation decoder
- `dex-swaps-pools/`: one shared no-vote block decoder feeding legacy and Hosted Stores
- `spkg/`: local SPKG dependencies used by aggregate packages

## Supported Sink Targets

- [x] [Substreams: File Sink](https://github.com/streamingfast/substreams-sink-files) (for map outputs)
- [x] [Substreams: SQL Sink](https://github.com/streamingfast/substreams-sink-sql) (ClickHouse packages in `svm-*/clickhouse`)
- [x] [Hosted Store](dex-swaps-pools/README.md) (verified pool identities)

The [`dex_swaps_pools`](dex-swaps-pools/README.md) package also exposes the legacy
`store_pools` key-value store and `map_pools` new-pool stream. Its shared
`map_pool_creations` output feeds both that store and the independent Hosted
Store producer, `map_pool_entries`.

## Aggregate Domain Packages (`svm-*`)

- [x] `svm-transfers`
- [x] `svm-balances`
- [x] `svm-accounts`
- [x] `svm-metadata`
- [x] `svm-native`
- [x] `svm-spl`
- [x] `svm-dex`
- [x] `svm-nfts`
- [x] `svm-staking`

## Supported DEXes and AMMs

Native support means swaps are decoded directly from on-chain program instructions.

| Program Name | Program ID |
|------------|--------------|
| Raydium Liquidity Pool V4 | 675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8 |
| Pump.fun AMM | pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA |
| Pump.fun | 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P |
| Raydium Concentrated Liquidity | CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK |
| Raydium CPMM | CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C |
| Meteora DLMM Program | LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo |
| Meteora Pools Program | Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB |
| Meteora DAMM v2 | cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG |
| Raydium Launchpad | LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj |
| Whirlpools Program | whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc |
| Lifinity Swap V2 | 2wT8Yq49kHgDzXuPxZSaeLaH1qbmGXtEyPy64bL7aD3c |
| Phoenix | PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY |
| Openbook V2 | opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb |
| stabble Weighted Swap | swapFpHZwjELNnjvThjajtiVmkz3yPQEHjLtka2fwHW |
| BonkSwap | BSwp6bEBihVLdqJRKGgzjcGLHkcTuzmSo1TQkHepzH8p |
| PancakeSwap | HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq |
| Moonshot | MoonCVVNZFSYkqNXP6bxHLPL6QQJiMagDL3qcqUQTrG |
| Darklake | darkr3FB87qAZmgLwKov6Hk9Yiah5UT4rUYu8Zhthw1 |
| Aldrin AMM V2 | CURVGoZn8zycx6FXwwevgBTB2gVvdbGTEpvMJDbgs2t4 |
| Serum DEX V3 | 9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin |
| SolFi | SoLFiHG9TfgtdUXUjWAxi3LtvYuFyDLVhBWxdMZxyCe |
| SolFi V2 | SV2EYYJyRz2YhfXwXnhNAevDEui5Q6yrfyo13WtupPF |
| Obric V2 | coUnmi3oBUtwtd9fU42p6jg75UmdnNMgBQMedGNYEAs |
| Obric V3 | ob2wXVFGiWXkPwYv8CSpPMm5m3NR7DjSrVJAHGSb9Gu |
| GoonFi | goonERTdGsjnkZqWuVjs73BZ3Pb9qoCUdBUL17BnS5j |
| Saros AMM | SSwapUtytfBdBn1b9NUGG6foMVPtcWgpRU32HToDUZr |
| Sanctum | 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx |
| DumpFun | DumpFunGAgW6kPHzWMA3Nnqecyrd6SGnLZvNGp2aHwEa |
| Boop | boop8hVGQGqehUK2iVEMEnMrL5RbjywRzHKBmBE7ry4 |
| Heaven | HEAVENoP2qxoeuF8Dj2oT1GHEnu49U5mJYkdeC8BAX2o |
| Plasma (Gavel) | srAMMzfVHVAtgSJc8iH6CfKzuWuUTzLHVCE81QU1rgi |
| Drift V2 | dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH |
| DFlow V4 | DF1ow4tspfHX9JwWJsAb9epbkA8hmpSEAtxXy1V27QBH |
| OKX DEX V2 | 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma |
| ByReal CLMM | REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2 |
| Jupiter Aggregator V4 | JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB |
| Jupiter Aggregator V6 | JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 |

## Quick Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

Most module directories also expose `make build`, `make pack`, `make noop`, and `make gui` targets.

## Rust SDK and protobuf models

New modules use Substreams 0.8, Solana 0.16 and Buffa-generated messages.
The protobuf wire schemas and existing Substreams module outputs remain the
compatibility contract. See [protobuf development](docs/protobuf.md) for pinned
code generation, message presence, enum handling and native decoder boundaries.
