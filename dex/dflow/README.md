# DFlow

This package captures routed swap activity from DFlow on Solana.

## Included events

- Swap instructions with input and quoted output amounts
- Source and destination token details for each routed trade
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
