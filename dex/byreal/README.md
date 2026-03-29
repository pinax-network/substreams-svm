# Byreal

This package captures concentrated liquidity swap activity from Byreal on Solana.

## Included events

- Swap instructions with input and minimum output amounts
- Pool and token account details needed to interpret each swap
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
