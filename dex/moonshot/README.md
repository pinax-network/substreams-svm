# Moonshot

This package captures trading activity from Moonshot on Solana.

## Included events

- Buy and sell events emitted by Moonshot
- Token amount details for each trade
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
