# Serum

This package captures trading activity from Serum markets on Solana.

## Included events

- Trade execution events created when orders match on Serum
- Base and quote amount details for each fill
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
