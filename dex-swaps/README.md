# DEX Swaps

`dex-swaps` emits a normalized protobuf stream of swap activity across supported Solana DEX protocols.

Each emitted swap includes:

- protocol enum
- protocol program ID (`amm`)
- pool or market identifier
- user wallet
- input token mint and amount
- output token mint and amount

Only protocols that can reliably provide the full normalized swap shape are included.

Notes:

- `protocol` is a protobuf enum with stable protocol IDs.
- Downstream packages such as `svm-dex` can materialize that enum into lowercase snake_case strings such as `jupiter_v6` and `raydium_amm_v4`.
- `amm` is the protocol program ID as raw bytes.
- `amm_pool` is the stable pool or market identifier when the protocol exposes one.
- If a protocol does not expose a pool identifier, `amm_pool` falls back to `amm`.
- Jupiter currently uses `fee_payer` as the best available user-level identity from the decoded event stream.

Current coverage:

- Boop
- Darklake
- DumpFun
- Jupiter v4
- Jupiter v6
- Orca Whirlpool `SwapV2`
- Pump.fun
- Pump.fun AMM
- Raydium AMM V4
- Raydium CLMM `V2`
- Raydium CPMM
- Raydium Launchpad
- Meteora AMM (mints from token balances, or from `InitializeAccount*` for a temporary token account)
- Meteora DAAM
- Meteora DLMM
- OKX DEX V2
- SPL Token Swap-compatible programs

Currently excluded:

- Saros
  Current decode exposes intent fields like `amount_in` and `minimum_amount_out`, but not the user, pool, mints, or finalized output amount needed for normalized swaps.
- Sanctum
  Current decode exposes only the swap amount and does not provide normalized pool, mint, or finalized output fields.
- DFlow
  Current decoder is a placeholder that treats any instruction as a swap and does not expose reliable normalized swap fields.

## Solana 0.16 and Buffa migration

The Rust API now uses `substreams-solana-core` 0.16 and Buffa 0.9.2, with
Substreams 0.8 for the default WASM feature. This is a Rust source compatibility
change: older Prost-generated `Block` and output types are different Rust types,
even when they represent the same protobuf messages. Native callers must update
their chain model and protobuf runtime together. The SPL projector's separate
legacy Prost adapter remains available; it does not make an old `Block` directly
acceptable to `dex-swaps::project`.

Message fields use `buffa::MessageField<T>` instead of `Option<T>`; use
`as_option()` to preserve absence and `MessageField::some(value)` to construct a
present message. Enum fields use `buffa::EnumValue<E>` instead of `i32`; use
`as_known()` for a recognized variant or `to_i32()` when preserving an unknown
numeric value. Encoding and decoding use the `buffa::Message` trait. See
[protobuf development](../docs/protobuf.md) for generation and presence rules.

The `.proto` package names, field numbers, enum IDs and output meanings remain
unchanged. The existing normalized swap rows, ordering and WASM module output
schema remain the compatibility contract. Captured output-byte goldens and
native/wrapper parity tests check this migration; they do not qualify untested
protocol behavior or a live provider.

The native `project` and `project_with_limits` APIs cover legacy, v0 and v1
Solana messages. A v1 message is covered with or without its inline compute
budget (`transaction_config`), which no adapter reads, and must have no address
table lookups. They return `ProjectError::UnsupportedMessage` for an explicit
version above 1, contradictory version metadata, a v1 message with lookups, or
inline transaction configuration on a legacy or v0 message, including in failed
transactions. An absent version with `versioned: true` remains supported for
older v0 producers. These checks prevent newly added upstream fields from being
silently discarded. They apply to the validated native API; the legacy
`map_events` wrapper never read the message version and retains its existing
filtering and extraction behavior. On the current block
`spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz`, 132 of 314
transactions are v1 messages; the native projection yields 61 rows, 10 of them
from v1 transactions, identical to the wrapper's.

The [Linux compatibility workflow](../.github/workflows/sdk-migration.yml) checks
formatting, clean regeneration with Buf 1.69.0, both native DEX feature modes,
native-token and svm-dex tests, and the complete release WASM workspace on Rust
1.85. Generated files are committed and checked for both modifications and new
untracked output; run `make -C proto protogen` to reproduce them locally.

## Native complete-block API

Native applications should depend on `dex-swaps` with `default-features = false`.
This uses `substreams-solana-core` (the same model and instruction views reexported
by `substreams-solana`) and avoids linking Substreams WASM allocator exports into
the host process. Native logging uses the standard `log` facade. The default
`substreams` feature retains the existing WASM entry point and Substreams logging;
`map_events` remains callable in both feature modes with the same error type and
output. `common` uses the same default feature for its database, `Clock` and scalar
helpers; its pure Solana helpers remain available without it. The SDK-free IDL
dependency is pinned to merged substreams-solana-idls `main`, which includes
[#148](https://github.com/pinax-network/substreams-solana-idls/pull/148) and
[#147](https://github.com/pinax-network/substreams-solana-idls/pull/147).

`project(&substreams_solana::pb::sf::solana::r#type::v1::Block)` is callable from
native Rust and returns a validated `Projection`. It borrows the existing decoded
block and calls the same protocol extraction as the WASM wrapper, with no store,
RPC request, or Substreams execution. `project_with_limits` additionally lets a
caller bound transactions, instructions, logs, log bytes, and output rows. The
structural validator is the workspace's native SPL projector (`spl/transfers`),
used with its fail-closed Buffa adapter.

Every projection includes a clock, even for an empty block. Missing block time
remains `None`, and a parent slot may skip several slots. Transactions carry their
original index in the unfiltered block. Every row adds:

- `SwapKind::PoolExecution` or `SwapKind::AggregatorSummary` (Jupiter v4/v6 and
  OKX); summaries must not also count as venue executions.
- `Provenance::Instruction` with the exact outer/inner **event instruction**
  position and an event index, or `Provenance::Log` with the original log index.
  An event instruction may be a self-CPI emitted by an earlier swap invocation.
  Log rows keep `Provenance::Log`; their proved invocation is reported separately
  in `invocation`, and no instruction position is ever derived from a log offset.
- `invocation`: the invocation that wrote the row's evidence. Every log line is
  written by the innermost invocation open at that point of the runtime trace,
  so when the recorded instruction walk and the invocation logs align exactly
  (program ids, `invoke [depth]` versus both the recorded stack height and the
  open invocation stack, every inner instruction inside its own outer
  instruction, no `invoke` line missing except for top-level signature
  precompiles, balanced outcomes each closing its own program, no
  `Log truncated`), each line has exactly one writer. Instruction rows report
  their event instruction.
  A log row reports the writer of its evidence line, and only when that
  writer's program is the row's `program_id`; otherwise, or when the trace is
  missing, truncated, ambiguous or inconsistent, it is `None`. For log rows this
  names the writer of the line, **not** the instruction the legacy adapter
  paired with it: `invocation`, `invocation_outcome` and `aggregator_route`
  describe the evidence writer, and only `verified_context` proves that the
  row's pool, user, mints and amounts belong to one instruction.
- `InvocationOutcome`: success is proven only when the recorded instructions,
  program invocation depths, and balanced outcome logs match exactly. A caught
  ancestor failure revokes an earlier successful event. Truncated, ambiguous, or
  missing traces remain `Unknown`. Log rows take the outcome of the invocation
  that wrote their evidence line (earlier revisions always reported `Unknown`
  for log rows). This proves the writer's outcome, not correctness of the
  protocol adapter's pool/amount association.
- `verified_context`: the swap instruction whose accounts, together with the
  row's own evidence, independently rebuild every row field, after proven
  success of that invocation and all its ancestors. `None` means unverified.
  It proves which instruction a row's fields came from, not what they mean: a
  verified row carries exactly the adapter's own fields, with their existing
  semantics.
  - Meteora DLMM instruction rows: the swap that made the event self-CPI (the
    parent of `invocation`), when its pool, user and token accounts match the
    event's intrinsic pool/user.
  - Orca Whirlpool, Raydium AMM v4, CLMM (swaps and router hops), CPMM and
    Meteora AMM log rows: `invocation` itself, when the adapter's own row
    builder, given that instruction and the row's evidence line, reproduces the
    row exactly (for Orca, the hop matching the event's pool and direction).
  - OKX summaries: `invocation` itself, when that `SwapV3` instruction, run
    through the adapter with only the log lines it wrote itself, completes the
    same summary on the same line. The summary keeps the adapter's meaning:
    `input_amount` is the instruction's `amount_in` and `output_amount` is the
    last `SwapEvent`'s `amount_out`. That is the route's final swap output when
    the route is one chain whose last hop uses one dex, even if an earlier hop
    is split. When the last hop is split across dexes, or the route has
    parallel chains, it is the output of one leg of the last hop or the last
    chain, and the row is still verified. Either way it is the logged dex leg
    output, not adjusted for any OKX commission or platform fee the
    instruction carries. An unverified OKX summary may carry another `SwapV3`'s user,
    mints and `input_amount`, even one in another route (see the count-once
    rule below).
  - Every other protocol (including Jupiter v4/v6 summaries): always `None`.

  Legacy log adapters pair events with instructions by sequence or FIFO, so a
  missing, extra or undecodable call, or an event-shaped line written by
  another program, shifts the pairing. The check proves each pairing against
  the invocation that actually wrote the event. A mispaired row is retained
  and never verified. Its `invocation` names the writer of its line only when
  that writer runs the row's program (for example a sequence shift between
  two calls of one program); a row built from a line that another program
  wrote has `invocation == None`.
- `aggregator_route`: the outermost Jupiter v4, Jupiter v6 or OKX invocation
  enclosing (or equal to) the row's `invocation`, i.e. the route the evidence
  writer ran in. Nested aggregators share the outermost route, so one route can
  hold summaries from several aggregators that describe the same hop (see
  below). Rows are never merged. `None` with a proved `invocation` means
  outside every route; `None` with `invocation == None` means route membership
  is **unknown**, not unrouted.
- `execution_ordinal`: the rank of the row's evidence in the proved trace, from
  0, across instruction and log rows: an instruction row at its event
  instruction's `invoke` line, a log row at its own line, then by event index.
  It orders evidence lines (an event self-CPI is written after the swap's own
  token transfers); it does not attribute the row, since an unattributed log
  row is ranked too. It is set for every row of a transaction exactly when that
  transaction's trace is proved, and for none otherwise. It is not a stable
  identity: a decoder change that adds or drops a row renumbers the rest.

**Counting a routed trade once.** Use only rows whose `invocation` is `Some`,
grouped by `(transaction_index, aggregator_route)`. For each route, count
exactly one of these two sets, never both and never a mix:

- its **legs**: the `PoolExecution` rows;
- its **route summaries**: the Jupiter v4 summaries whose `invocation` equals
  `aggregator_route` and the OKX summaries whose `verified_context` equals
  `aggregator_route` (written, and for OKX also rebuilt, by the route
  invocation itself), or, when the route has none of these, its Jupiter v6
  summaries.

A route without route summaries is counted by its legs.

An OKX summary needs `verified_context`, not just `invocation`. The adapter
completes the oldest pending `SwapV3` on any OKX `SwapEvent` line, so an
unverified OKX summary can carry another route's `SwapV3` fields (user, mints,
`input_amount`) while its `invocation` and `aggregator_route` name the route
that wrote the line. When OKX#0 writes one more `SwapEvent` than its route has
hops, that line completes OKX#1's summary inside route 0, and OKX#1's own line
then completes nothing: route 0 holds its own verified summary and OKX#1's
unverified one, and route 1 only its leg. Taken as a route summary, the
unverified one would count route 0 as two trades, and OKX#1's trade a second
time next to route 1's leg. Jupiter v4 and v6 summaries take their amounts and
mints from one event written inside their route (for Jupiter v4, by the route
itself), so they need no verifier for this.

Summaries of an aggregator nested inside the route describe the same hops
again, so the rule never counts them on top of the route's own. With Jupiter
v6 -> OKX `SwapV3` -> venue, the OKX summary is proved and even verified, but
against the nested OKX call, not the route, so the route summaries are the
Jupiter v6 ones. With OKX `SwapV3` -> Jupiter v6 -> venue they are the OKX
summary alone, or the Jupiter v6 summaries if the OKX summary is unverified.
Jupiter v6 summaries are event self-CPI instruction rows, so their `invocation`
is never the route itself; they are counted only in a route with no Jupiter v4
or verified OKX summary of its own, where they overlap no other counted
summary. The fields cannot separate a Jupiter v6 route from another Jupiter v6
route that it invokes directly, the only way a program can re-enter itself;
count legs if that matters.

Choose the set per route: on the pinned blocks summaries and decoded legs do
not correspond one to one (some routes have hops without a decoded leg, others
legs without a decoded summary). Different choices for different routes do not
double count, because every row the rule counts for a route is evidence
written inside it: a leg's event, a Jupiter summary's event, or an OKX summary
rebuilt from the route's own instruction and lines. Rows whose `invocation` is
`None` have unknown route membership and cannot be deduplicated by route. Where
a protocol has a verifier, require `verified_context` before trusting that a
row's fields came from one instruction; it does not change what a field means
(see OKX above).

**Row identity.** A row is identified by its `provenance`: instruction position
and event index, or log index and event index. Neither `invocation` (one
invocation can write several log rows) nor `execution_ordinal` is an identity.

**Source compatibility.** `ProjectedSwap` gained the public fields `invocation`,
`aggregator_route` and `execution_ordinal`. This is additive for code that only
reads fields, but source-breaking for struct literals and exhaustive patterns:
treat the update as a minor (pre-1.0 breaking) bump and add
`invocation: None, aggregator_route: None, execution_ordinal: None` to such
literals. Meanings also changed: log rows now report a proved
`invocation_outcome`, `verified_context` is no longer DLMM-only (filter on
`protocol` if you relied on that), and a `Log truncated` marker, a
ComputeBudget instruction without its `invoke` line, or an inner instruction
logged outside its own outer instruction now leaves the whole transaction
unproved.

`Projection::into_legacy()` strips only the added evidence. For a complete valid
block it yields the same row fields and order as `map_events`; the WASM wrapper
keeps its existing output and filtering behavior. The `swaps` vector keeps the
legacy adapter order (instruction rows, then log rows); sort by
`execution_ordinal` for the order of their evidence in the proved trace.

The native API does **not** repair or qualify existing decoder behavior.
SPL Token Swap's current output amount is the instruction minimum, not proven
realized output. Consumers requiring execution association must additionally
require a verified context, not just successful event invocation.
[`fixtures/native-execution-coverage.txt`](fixtures/native-execution-coverage.txt)
records per-protocol attribution, verification, ordering and routing counts on
the four pinned mainnet blocks. Those fixtures are offline evidence, not broad
protocol or live-provider coverage.

Tests compare the native and WASM-wrapper body, including encoded-output hashes
of all four pinned blocks captured from unchanged main. They also cover original
positions, aggregator classification, caught CPI failures, ambiguous/truncated
invocation traces (including an invocation logged at a depth the open stack
does not allow, a non-precompile instruction without its `invoke` line anywhere
in the sequence, and an outcome line closing another program), exact runtime
log grammar, failed-prior-swap FIFO association, malformed blocks, resource
bounds, empty blocks, and missing time.
Execution fixtures, all through `project()`, cover interleaved instruction and
log rows, a multi-hop instruction, nested CPI inside an aggregator route, failed
invocations and failed ancestors, event lines written by another program
(inside and outside a route), forged runtime text, sibling calls to one
program, and truncated, unbalanced, misaligned, height-less or out-of-place
traces. Every log verifier (Orca, Raydium AMM v4, CLMM swaps and router hops,
CPMM, Meteora AMM, OKX) has a positive fixture and stale-pairing fixtures
(sequence shifts from undecodable, missing or extra calls; a router payer or
stack-height mismatch; OKX forged, extra and non-`SwapV3` events) that must stay
unverified; replacing any verifier with `true` fails a test. Further stale
pairings differ from the writer's own row in exactly one field the verifier
takes from the instruction: the pool, the user, the stack height or the mints
(opposite-direction round trips for CLMM `swap_v2`, CPMM and Meteora AMM; for
OKX, a `SwapV3` that writes no event, so the next one's event completes its
queued context). Each verifier must therefore compare each of those fields.
Fields the paired row and the writer's rebuild both read from the same evidence
line cannot differ and are not separately tested: the amounts, Orca's pool and
mints (it pairs by them), CLMM router pools and mints, AMM v4 mints and the OKX
pool. An OKX summary must verify at the line that completes it; and a Meteora DLMM context must not
verify under a failed outer invocation. Jupiter v4 and OKX fixtures check that a
summary is attributed to its aggregator invocation, shares its route with the
venue leg and is ordered after it. A nested Jupiter v6 -> OKX `SwapV3` -> Orca
route and its reverse check the count-once rule, as do two OKX routes whose
first writes an extra `SwapEvent`: its leaked, unverified summary of the second
route's `SwapV3` is no route summary, and the second route falls back to its
leg. Split-hop and parallel-chain OKX routes pin that a verified summary keeps
the last event's output.
