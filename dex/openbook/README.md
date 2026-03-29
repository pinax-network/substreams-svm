# OpenBook

This package captures trade execution activity from OpenBook markets on Solana.

## Included events

- Fill events created when resting and taker orders match
- Total order fill summaries when a single order matches multiple times
- Transaction and instruction metadata needed to place the event in context

## Output

- This package exposes a map_events module with decoded protocol events.
- The output is designed to describe what happened on-chain in user-friendly terms, with transaction and instruction context where available.
