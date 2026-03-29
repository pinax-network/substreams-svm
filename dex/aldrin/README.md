# Aldrin

This package captures Aldrin swap activity on Solana.

## Included events

- Swap instructions with input and minimum output amounts
- Transaction context such as signature, fee payer, fees, and compute usage
- Instruction context such as program id and stack height

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
