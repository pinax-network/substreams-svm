# Native Token

> System program instructions related to transfers and Pre-Post Balances.

## Includes

### Transfers

- [x] `Transfer`
- [x] `TransferWithSeed`
- [x] `CreateAccount`
- [x] `CreateAccountWithSeed`
- [x] `WithdrawNonceAccount`

### Balances

- [x] `PreBalance`
- [x] `PostBalance`

### Builtins / Loaders / Utilities Program IDs

- [x] `11111111111111111111111111111111` (System Program)
- [x] `ComputeBudget111111111111111111111111111111`
- [x] `BPFLoaderUpgradeab1e111111111111111111111111`
- [x] `AddressLookupTab1e1111111111111111111111111`
- [x] `Ed25519SigVerify111111111111111111111111111`
- [x] `KeccakSecp256k11111111111111111111111111111`

### Stake Program ID

- [x] `Stake11111111111111111111111111111111111111`

## Native Firehose projection

Depend on `native-token` with `default-features = false` and call
`project(&block)` or `project_with_limits(&block, limits)` on an already decoded
`substreams-solana-core` block. The dependency graph excludes the Substreams
runtime; the default feature preserves the WASM wrapper.

The projector shares the wrapper's system instruction decoder, validates complete
block structure and legacy/v0 message support, and returns original transaction,
outer instruction and inner instruction positions. It resolves loaded accounts
and skips failed transactions. Results borrow transaction metadata. Input bytes
and protobuf allocations must also be bounded by the caller. Capacity or malformed
input returns an error, never a partial prefix.

Coverage is positive-lamport Transfer, TransferWithSeed, CreateAccount,
CreateAccountWithSeed and WithdrawNonceAccount. Zero amounts are omitted; self
transfers remain observed instructions. Fees, rewards, stake instructions and
arbitrary account balance changes are not transfers. In the existing database
formatter, only ordinary Transfer feeds `system_transfer`; seeded transfers and
nonce withdrawals have separate tables. Do not combine those row contracts.

These are observed instructions from successful transactions. Inner instructions
can fail and be caught; this API does not yet prove each invocation's success or
report net balance changes. That proof is a serving qualification requirement.
The checked native API rejects malformed system instructions; the legacy wrapper
skips malformed encodings/account lists instead of indexing missing accounts.

Full SPL extraction is separately available from `spl-transfers::project` with
`Coverage::ALL` over a borrowed view of the same block. It covers the supported
Token/Token-2022 transfer, mint and burn variants, including inner instructions;
unchecked mint/decimals can remain unknown. Token-2022 extension-specific variants
and net amounts after transfer fees are not implied by `ALL`.
