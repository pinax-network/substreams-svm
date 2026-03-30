# DEX Swaps

`dex-swaps` emits a normalized protobuf stream of swap activity across supported Solana DEX protocols.

Each emitted swap includes:

- protocol enum
- protocol program ID (`amm`)
- pool or market identifier
- user wallet
- input token mint and amount
- output token mint and amount

Only protocols that can reliably provide the full normalized swap shape are included.

Notes:

- `protocol` is a protobuf enum with stable protocol IDs.
- Downstream packages such as `svm-dex` can materialize that enum into lowercase snake_case strings such as `jupiter_v6` and `raydium_amm_v4`.
- `amm` is the protocol program ID as raw bytes.
- `amm_pool` is the stable pool or market identifier when the protocol exposes one.
- If a protocol does not expose a pool identifier, `amm_pool` falls back to `amm`.
- Jupiter currently uses `fee_payer` as the best available user-level identity from the decoded event stream.
- Meteora AMM is intentionally excluded for now because its current decoded swap shape does not expose canonical token mint identities for normalization.

Current coverage:

- Boop
- Darklake
- DumpFun
- Jupiter v4
- Jupiter v6
- Orca Whirlpool `SwapV2`
- Pump.fun
- Pump.fun AMM
- Raydium AMM V4
- Raydium CLMM `V2`
- Raydium CPMM
- Raydium Launchpad
- Meteora DAAM
- Meteora DLMM
