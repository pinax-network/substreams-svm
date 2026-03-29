# Jupiter V6

This package captures routed swap activity from Jupiter V6 on Solana.

## Included events

- Routed swap events across supported liquidity sources
- Input, output, and route token amount details for each trade
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
