# Migration of closed token accounts to svm-balances v0.3.5 (2026-11-02)

Record of the one-time migration of `solana:svm-balances@v0.3.3` on `tokenapis-b` after the
live sink moved to [svm-balances v0.3.5](https://github.com/pinax-network/substreams-svm/releases/tag/svm-balances-v0.3.5)
(#241, fixing #240). The two scripts beside this file are what ran; the live sink was never
paused. All times UTC.

## Why

`balances` kept the last positive amount of every token account that was closed, because the
sink wrote balances from each transaction's post-token-balances and a closed account has none.
On the Token API's `/v1/svm/holders`, 877 of wSOL's and 389 of USDC's top 1,000 holders were
closed accounts. v0.3.5 writes a zero balance at the close; this migration wrote the zeros for
the closes before it.

## Timeline

| Time | Step |
| --- | --- |
| 17:07 | Live sink restarted on v0.3.5 against the same database, cursor continued from the head ([k8s-token-api-services#59](https://github.com/pinax-network/k8s-token-api-services/pull/59), then [#60](https://github.com/pinax-network/k8s-token-api-services/pull/60) back to `--on-module-hash-mistmatch=error`). 400 post-switch closes checked: all as the new rule prescribes. |
| 21:16 | `migrate-v0.3.5-closed-accounts.sh`, `ONLY_PREFIX=Ab`: capture view created, boundary `D = 464643104`, `mv_historical_balances_state` detached. The chunk took 38 s for 1.29 M rows with `max_insert_threads=4` and 1 M-row blocks: one sink thread wrote the big block in 14 s, the copy into `balances_by_account` ran serially for 7 s, the ReplacingMergeTree's on-insert dedup spent 6 s on rows without duplicates. |
| 21:30 | `ONLY_PREFIX=Ac` with `parallel_view_processing=1` and `optimize_on_insert=0`: 5 s for the same 1.29 M rows. |
| 21:30 | Full run, 4 chunks in parallel. The first setting used 200 K-row insert blocks, 7 parts per chunk per table: active parts of `balances` went from 22 to 640 in five minutes, toward `parts_to_delay_insert = 1000`, where every insert into the table is delayed, the sink's included. |
| 21:35 | Ctrl-C and rerun with 1 M-row blocks (2 parts per chunk per table). Parts fell to 160 within four minutes and stayed between 40 and 160. Chunks took 15 to 20 s, about 13 a minute. |
| 22:39 | All 3,364 chunks done: 1,262 M zero rows. View re-attached, capture view dropped, 10,089,330 captured sink rows replayed into `historical_balances_state` in 12.7 s, capture table dropped. |
| 22:45 | Check: USDC clean in the data; wSOL still had 848 closed accounts in its top 1,000, every one closed in the same block as its last positive row (the temporary wSOL account pattern), which the first pass's `close_block > last_balance_block` skipped. |
| 22:59 to 23:20 | `migrate-v0.3.5-same-block.sh`: 76,571,445 zero rows at the stale row's own block number, view attached, 1 to 5 s per chunk. |
| 23:35 | Check on the view's current top 1,000 (last refreshed before the passes): 877 wSOL and 232 USDC accounts are closed, and `balances_by_account FINAL` has none of them with a positive amount. The next hourly refresh drops them. |

Throughout: sink lag 2 to 11 s, no delayed or rejected insert on either node, load average on the
writer node 42 to 58 on 36 cores. Nothing was written to `historical_balances_state` for the
first pass's rows (nothing reads it for Solana in pinax-api 3.25.1); the second pass's rows did
reach it through the attached view.

## Design notes

- **Candidates.** For each (account, program_id, mint) with a positive latest amount in
  `balances_by_account`, the account's latest state in `solana:svm-accounts@v0.3.1.close_account_state`
  must be closed, with the close at a later block (first pass) or the same block (second pass).
  An account re-opened after a close has a later open state and is never touched. Chunks are
  two-character base58 account prefixes; the mass sits under first characters `1` to `9` and
  `A` to `H`, about 986 heavy chunks of 1.3 M rows.
- **Why the historical view was detached.** Each zero row fans out into 8 rows of
  `historical_balances_state` (77.6 B rows, 5.8 TiB): about 10 B rows and 0.8 TiB per replica
  for data no route reads.
- **Why the sink's rows were captured.** They cannot be replayed from `balances` afterwards:
  it is a ReplacingMergeTree, so merges keep only an account's latest row and the window's
  history is gone. A temporary view copied every row with a `block_hash` (sink rows always have
  one, migration rows never) into a plain replicated table; the replay ran the historical
  view's own query over it for blocks after `D`. D is taken before the detach and the capture
  view is dropped after the re-attach, so no row is missed; the sink's inserts in flight at
  those two moments are aggregated twice, which only affects the `transactions` sum.
- **Same-block zeros.** The second pass inserts the zero with the stale row's block number.
  Among equal versions a ReplacingMergeTree keeps the later-inserted row, so the zero wins in
  `balances` and marks the account deleted in `balances_by_account`.
- **Parts.** With one partition per table, the part creation rate is chunks per minute times
  parts per chunk per table. Keep insert blocks at 1 M rows; the merge pool (16 threads)
  absorbed 2 parts per chunk at 13 chunks a minute with room to spare and did not absorb 7.
