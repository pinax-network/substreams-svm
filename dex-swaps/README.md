# DEX Swaps

`dex-swaps` emits a normalized protobuf stream of swap activity across supported Solana DEX protocols.

Each emitted swap includes:

- protocol identifier
- pool or market identifier
- user wallet
- input token mint and amount
- output token mint and amount

Only protocols that can reliably provide the full normalized swap shape are included.
