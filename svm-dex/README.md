# SVM DEX

This package combines supported Solana DEX protocol events into one database-ready stream.

## Included events

- Normalized swap, buy, sell, and fill style events across supported DEX protocols
- Shared transaction, block, and instruction context for downstream analytics
- One combined db_out output for database sinks and analytics pipelines

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
