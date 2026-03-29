# Obric V2

This package captures Obric V2 swap activity on Solana.

## Included events

- Swap events emitted by Obric V2
- Token movement details for each trade
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
