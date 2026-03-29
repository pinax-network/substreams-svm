# OKX DEX

This package captures routed swap activity from the OKX DEX router on Solana.

## Included events

- Routed swap events executed through OKX DEX
- Input and expected output token details for each trade
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
