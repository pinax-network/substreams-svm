# Pool creation decoder coverage

This is the pool-creation coverage checklist for all 37 protocol packages under
`dex/`, plus SPL Token Swap, as inspected on 2026-10-26. A protocol appearing in
the repository's swap-decoder list does not establish creation-decoder coverage.
The implementation currently covers 25 protocol families, with nine remaining
coverage gaps and four routing packages. The gaps include Sanctum's mutable
shared pool and Drift's market model, which require additional data contracts;
neither is counted as covered or not applicable. This does not provide a complete historical registry of
every advertised DEX or every version of an implemented protocol.

The source of implemented coverage is [`dex-pools`](../dex-pools/src/lib.rs),
its [matched-event decoder](../dex-pools/src/decode.rs), and its
[initialization adapters](../dex-pools/src/initialization/mod.rs). The shared IDL
baseline for the gaps below is
[`substreams-solana-idls` at `6650358`](https://github.com/pinax-network/substreams-solana-idls/tree/665035860128e892504c37e2bfa7b7b96063cd42/src).
Update this checklist with adapter, upstream layout, and fixture changes.

## Admission meanings

- **Matched event**: a typed initialization and matching event are attributed to
  the correct invocation; that invocation and all ancestors must succeed.
- **Initialization**: the typed initialization account layout supplies identity;
  the exact initialization invocation and all ancestors must succeed. An event
  and its additional fields are not implied. `Creation.evidence` records this
  explicitly as `Initialization`.
- **Gap**: no creation record is currently emitted for this protocol or variant.
  The listed missing parser, account layout, or identity evidence must be
  implemented and tested before changing the status.
- **Routing**: do not manufacture a pool from the router's program ID or from
  one arbitrary account. Decode an underlying venue's own creation.
- **Gap; different contract**: the program has its own pool or market state,
  but the current immutable asset-list contract does not represent it correctly.
  This remains unimplemented coverage, not a router exemption.

All raw decoding functions return an **unverified `Candidate`**. Account and
instruction decoding cannot itself prove that the program accepted the payload
or that an ancestor did not later fail. Only the full-block projector applies
execution admission. Public struct construction is not proof either.

## Protocol checklist

| Package | Current creation status | Identity and variants, or remaining work |
| --- | --- | --- |
| `dex/aldrin` | Initialization with token-account context | AMM V2 `Initialize`; pool identity and base/quote vault order. Underlying mints require unambiguous metadata for those exact vaults, or the contextual raw API. The official V2 SDK binds the layout to the declared `CURVGo…` program, and the captured successful initializer at slot 193097357 verifies that binding. The legacy CLOB compatibility parser is not used for pool creation. |
| `dex/bonk-swap` | Initialization | `CreatePool`; pool account, X/Y mint order and token custody accounts. Decimals remain absent. |
| `dex/boop` | Initialization | `DeployBondingCurve` and `DeployBondingCurveFallback`; bonding-curve identity, mint/native-SOL order. Token creation alone is not pool creation. A downstream Meteora/Raydium pool belongs to that AMM's own initialization. |
| `dex/byreal` | Initialization | `CreatePool`; pool-state identity, mint0/mint1 order, separate token programs and custody vaults. |
| `dex/darklake` | Initialization | `InitializePool`; exact 25-account layout, pool identity and X/Y order. Additional WSOL fee reserves and LP mint are not underlying X/Y assets. |
| `dex/dflow` | Routing | No own AMM pool identity is established. The existing instruction parser is insufficient evidence for venue creation. |
| `dex/drift` | Gap; different contract | The selected `dRifty…` program owns spot and perpetual markets. `InitializeSpotMarket` establishes one SPL mint with custody and same-mint insurance vaults; `InitializePerpMarket` establishes derivative/oracle state with numeric AMM reserves, not a pair of SPL token vaults. Market identity, asset linkage and market kind need an explicit contract; `BeginSwap`/`EndSwap` do not provide it. |
| `dex/dumpfun` | Initialization | `Mint`; liquidity-pool identity and mint/native-SOL order. Token decimals remain absent; native lamports are not represented as a token vault. |
| `dex/goonfi` | Gap | Shared instruction parser exposes swaps only. |
| `dex/heaven` | Gap | Shared instruction parser exposes `Sell` only. |
| `dex/jupiter/v4` | Routing | Aggregator routes have no own canonical venue pool. Underlying venue initializations are decoded independently. |
| `dex/jupiter/v6` | Routing | Aggregator routes have no own canonical venue pool. `CreateOpenOrders` and token-account helpers are not pool creation. |
| `dex/lifinity` | Gap | Shared parser exposes swap/deposit/withdraw, without pool initialization. |
| `dex/meteora/amm` | Initialization | All six existing shared initializers: permissioned; permissionless; permissionless with fee tier; constant product with config; config2; customizable constant product. Pool identity and A/B order. The permissioned layout does not expose token custody accounts, so vaults remain absent rather than using vault-program state accounts. |
| `dex/meteora/daam` | Initialization | `InitializeCustomizablePool`, `InitializePool`, `InitializePoolWithDynamicConfig`; pool identity and A/B order. The shared IDL also exposes `EvtInitializePool`, but initialization-only records do not imply event-derived decimals. |
| `dex/meteora/dlmm` | Initialization | `InitializeLbPair`, `InitializeLbPair2`, both customizable permissionless variants, and `InitializePermissionLbPair`; pair identity and X/Y order. `LbPairCreate` is not decoded by the pinned shared event parser; creation admission here uses the proved initializer. |
| `dex/moonshot` | Initialization | `TokenMint`; curve-account identity and mint/native-SOL order, base decimals from initialization. Only the shared `Currency::Sol` value is admitted; unknown future currencies are omitted. |
| `dex/obric-v2` | Gap | `CreatePair`/`CreatePairV2` tags exist; shared account helpers cover swaps, not creation. |
| `dex/obric-v3` | Gap | `CreatePair` exists; shared account helpers cover swaps, not creation. |
| `dex/okx-dex` | Routing | Swap router, without an own venue-pool creation contract. |
| `dex/openbook` | Initialization; partial raw parser | `CreateMarket`; market identity and base/quote order. The shared IDL recognizes its discriminator and account layout but does **not** deserialize creation arguments. Raw candidates do not claim argument validity; committed records require proof that the real program accepted the invocation. |
| `dex/orca/whirlpool` | Matched event | `InitializePool`, `InitializePoolV2`, `InitializePoolWithAdaptiveFee` paired with the same invocation's `PoolInitialized`; Whirlpool identity and A/B order. Historical calls without the required event remain omitted. |
| `dex/pancakeswap` | Initialization | `CreatePool`; pool-state identity, mint0/mint1 order, separate token programs and custody vaults. |
| `dex/phoenix` | Initialization; order-book market | Typed `InitializeMarket`; market identity and base/quote order with custody vaults. This is not an AMM pool. |
| `dex/plasma` | Initialization | `InitializePool`; exact ten-account layout, pool identity and base/quote order. Self-program account must match the invoked program. |
| `dex/pumpfun/amm` | Matched event | `CreatePoolV2` and `CreatePoolV3`, each with the matching same-version creation self-CPI; pool identity and base/quote order. V1 remains unsupported by the shared parser. |
| `dex/pumpfun/bonding_curve` | Initialization | Historical `Create` and Token2022 `CreateV2`; bonding-curve identity, minted token followed by quote. Historical creation quotes native SOL. V2 uses native SOL only with no remaining quote accounts; otherwise the shared helper requires explicit quote mint, custody account and token program. Partial quote groups are rejected. |
| `dex/raydium/amm-v4` | Initialization | Historical `Initialize` and `Initialize2`; AMM account identity and coin/pc order. |
| `dex/raydium/clmm` | Initialization | `CreatePool`, `CreateCustomizablePool`, `CreateCustomizablePoolV2`, `CreatePermissionedPool`; each uses its shared account layout, pool-state identity and mint0/mint1 order. |
| `dex/raydium/cpmm` | Initialization | `Initialize` and `InitializeWithPermission`; pool-state identity and token0/token1 order. |
| `dex/raydium/launchpad` | Initialization | `Initialize`, `InitializeV2`, `InitializeWithToken2022`; pool-state identity and base/quote order. Base decimals come from initialization, including a present zero; quote decimals remain absent. |
| `dex/sanctum` | Gap; different contract | The selected `5ocn…` program is Infinity's S Controller and owns a shared multi-LST pool. `Initialize` creates pool state and configures the LP mint; separate `AddLst`/`RemoveLst` instructions change underlying membership. Creation decoding plus membership history or a qualified snapshot is required; freezing one asset list on first write cannot represent all its assets. The existing swap-via-stake parser is insufficient. |
| `dex/saros` | Initialization with token-account context | Typed `Initialize`; pool identity and A/B vault order. Mints must be established for those exact vaults by unambiguous transaction token-account metadata, or supplied to the contextual raw API. Missing/conflicting context omits the candidate. |
| `dex/serum` | Initialization; order-book market | Strict canonical version-0 `InitializeMarket` decoder and at least ten accounts, including rent. Market identity and coin/price order. Mint decimals and token-program metadata absent from initialization remain absent. The old raw-payload parser is not used. |
| `dex/solfi-v1` | Gap | Shared instruction parser exposes swaps only. |
| `dex/solfi-v2` | Gap | Shared instruction parser exposes swaps only. |
| `dex/stabble` | Initialization; multiple assets | Typed `Initialize`; all remaining underlying mint accounts in weight order, with mint/weight/cap counts required to agree. The fixed prefix's LP mint and vault-program state are excluded from underlying assets. Unknown custody vaults, programs and decimals remain absent. |
| `spl/token-swap` | Initialization with token-account context | Strict initialization decoder plus typed pool/vault accounts. Underlying mints require unambiguous metadata for those exact vaults, or the contextual raw API. Dispatch is restricted to the canonical program allowlist. |

The shared IDL also contains Raydium Stable initialization types. There is no
corresponding advertised `dex/raydium/stable` package in this checklist; adding
that program is an explicit coverage change.

### Missing creation contracts

The final source audit did not qualify additional initializers for the seven
layout gaps. Lifinity's
[`@lifinity/sdk-v2@2.0.3` IDL](https://unpkg.com/@lifinity/sdk-v2@2.0.3/lib/idl/lifinity_amm_v2.d.ts)
and its program-owned on-chain IDL expose swap, deposit and withdrawal only.
The published `obric-solana@1.1.0` package includes admin constructors, but binds
them to different program addresses (`obriQD…` / `obr3KR…`) from the selected
Obric programs (`coUnmi…` / `ob2wXV…`); those layouts cannot establish these
programs' creation accounts. SolFi and GoonFi swap integrations do not establish
initialization layouts, and newer GoonFi V2 references concern a different
program. Heaven's
[official integration repository](https://github.com/heavenxyz/heaven-integration/blob/b963f63cab94ba6e28c98b5fb3975e9edd4af9cd/README.md)
describes private integration.

A finalized RPC audit at slot 461787019 confirmed Lifinity's limited IDL and
found no canonical Anchor IDL account for the other six selected programs.
Missing IDL accounts do not prove that initialization is impossible. Closing
these gaps requires an exact-program creation contract or independently
qualified historical creation evidence; copying another deployment's layout
or inferring pool identity from a swap is insufficient.

### Programs requiring different contracts

Sanctum's [official deployed-program list](https://learn.sanctum.so/docs/for-developers/deployed-programs)
binds `5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx` to S Controller, and its
[Infinity documentation](https://learn.sanctum.so/docs/technical-documentation/infinity)
identifies the shared pool-state PDA, LST membership list and per-LST reserves.
The historical controller's
[`Initialize`](https://github.com/igneous-labs/S/blob/66de438b9e049aacc193b5795d26ac055d86d770/programs/s-controller/src/processor/initialize.rs),
[`AddLst`](https://github.com/igneous-labs/S/blob/66de438b9e049aacc193b5795d26ac055d86d770/programs/s-controller/src/processor/add_lst.rs)
and [`RemoveLst`](https://github.com/igneous-labs/S/blob/66de438b9e049aacc193b5795d26ac055d86d770/programs/s-controller/src/processor/remove_lst.rs)
establish that membership is separate from creation. Current development is in
[`inf-1.5`](https://github.com/igneous-labs/inf-1.5/tree/d4bfdd4d1d34985f8639433c0308941fc84bf1a2).
This is an unimplemented pool, not an underlying stake-pool or router substitute.

Drift's [program declaration](https://github.com/drift-labs/protocol-v2/blob/13e8e9b8d614f3b62e3a65a8c372c819e6529aeb/programs/drift/src/lib.rs)
matches `dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH`. Its
[initialization handlers and account types](https://github.com/drift-labs/protocol-v2/blob/13e8e9b8d614f3b62e3a65a8c372c819e6529aeb/programs/drift/src/instructions/admin.rs)
distinguish a single-asset spot market from a perpetual market backed by oracle
and collateral accounting. Treating an oracle, insurance vault or arbitrary
quote as a second underlying SPL mint would invent asset identity. Supporting
these markets remains separate work under the request for every advertised DEX.

## Native assets, fields, and store behavior

Token order is the protocol's creation order, never a later swap's input/output
order. `tokens` is a vector so multi-asset pools retain all assets. Missing
decimals, custody vaults or token programs remain absent. The
initializer must establish mint identity; an unknown or program address is not
substituted for it.

Historical Pump `create`, SOL-quoted `create_v2`, Moonshot, Boop and Dumpfun
creation records use native-SOL quotes. A native-SOL record uses
the repository's normalized SOL asset identifier,
`So11111111111111111111111111111111111111112`, and **`is_native = true`**. Those
bytes are also the canonical wrapped-SOL mint; the flag distinguishes native
lamports from an SPL token holding. A native-SOL record has no SPL token-program
or token-vault claim. The official [Pump program documentation](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_PROGRAM_README.md)
and [instruction IDL](https://github.com/pump-fun/pump-public-docs/blob/main/idl/pump.json)
define the distinct Pump creation layouts and quote selection. Current V2's
remaining accounts may select another token as quote; the decoder never
silently substitutes SOL for a malformed or incomplete quote group. The raw
initialization API permits up to 255 resolved accounts, including longer quote
chains; the matched-event API retains its existing 32-account bound.

Boop and Dumpfun account helpers come from the program-owned Anchor IDL accounts
for the exact declared program IDs, with provenance retained in the
[upstream creation fixtures](https://github.com/pinax-network/substreams-solana-idls/tree/665035860128e892504c37e2bfa7b7b96063cd42/tests/fixtures/pool-initialization).
Their SOL reserve fields establish native quote semantics. These IDL provenance
checks qualify layouts; they are not captured successful creation transactions.

Aldrin's [retained source evidence](https://github.com/pinax-network/substreams-solana-idls/blob/665035860128e892504c37e2bfa7b7b96063cd42/tests/fixtures/pool-initialization/aldrin-initialize.json)
links the official V2 registry layout, SDK program binding and captured successful
transaction. The [native captured fixture](../dex-pools/fixtures/README.md)
preserves its complete selected transaction, including the exact vault token
balances used to resolve base and quote assets. Missing or conflicting vault
context omits the candidate; the raw API without context cannot decode it.

Every admitted native record is exported to the shared `Pools` message and both
store branches. Hosted Store uses the raw 32-byte identity; legacy storage uses
canonical base58 of the same bytes. Values preserve the same encoded `Pool`
payload. Both branches keep the first value, so optional metadata left absent
at creation is not automatically enriched by later observations. This registry
does not track reserves, fees, ownership changes or closures.

## Completeness and evidence limits

The configured source is `solana_common:blocks_without_votes`. It removes
transactions containing the static vote-program key, including transactions
that merely carry that account. Native full-block projection does not apply
this filter. Therefore the two input modes can differ in retained creations;
their transaction indexes also differ. Use signature and instruction/log
positions for source identity.

Missing, truncated, inconsistent or failed invocation traces do not establish
committed pools. Unknown layouts are omitted, while malformed supported block
structure and capacity violations fail the projection. Empty output is not
evidence that no pools were created.

An historical deployment only knows creations in its processed range and
qualified layouts. Beginning at a recent slot cannot discover older inactive
pools. Snapshot bootstrap, upgrades to frozen optional fields, closed/recreated
accounts, and cross-block metadata conflicts need an explicit registry policy.

## Validation evidence

[`projection.rs`](../dex-pools/tests/projection.rs),
[`captured.rs`](../dex-pools/tests/captured.rs),
[`other_venues.rs`](../dex-pools/tests/other_venues.rs),
[`additional_venues.rs`](../dex-pools/tests/additional_venues.rs),
[`aldrin.rs`](../dex-pools/tests/aldrin.rs),
[`raydium_meteora.rs`](../dex-pools/tests/raydium_meteora.rs), and
[`contextual_initializers.rs`](../dex-pools/tests/contextual_initializers.rs)
cover their stated variants. All 43 native tests passed against the shared IDL
revision above. Synthetic typed-layout fixtures demonstrate
decoding and execution-admission logic; they do not qualify every
historical/current deployment. Captured-chain evidence is listed separately in
[`dex-pools/fixtures`](../dex-pools/fixtures).

The independent [native consumer tests](../dex-pools/consumer-tests/README.md)
demonstrate borrowed decoding from Buffa and Prost `Bytes` without a Substreams
runtime. The [packed wrapper tests](../dex-swaps-pools/tests/package.rs) verify
the configured no-vote source and both storage paths. Rebuild and qualify the
package after coverage changes before claiming its committed WASM has the new
coverage.
