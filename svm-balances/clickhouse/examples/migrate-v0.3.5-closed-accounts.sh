#!/usr/bin/env bash
# One-time migration for svm-balances v0.3.5 (pinax-network/substreams-svm#241) on the live
# `solana:svm-balances@v0.3.3` database: inserts a zero `balances` row for every (mint, account)
# whose latest balance is positive but whose account's latest close in `solana:svm-accounts@v0.3.1`
# is newer than that balance. Run after the live sink runs v0.3.5 (closes since the switch are
# already 0 and skipped here). The live sink keeps running throughout; it is never paused.
#
# The historical view is detached for the run, so the migration's rows (about 1.3 B, 8 historical
# rows each) never reach historical_balances_state, which no Solana route of the Token API reads
# (pinax-api 3.25.1). The rows the sink writes meanwhile are captured by a temporary view into a
# plain table and replayed into the historical state afterwards. They cannot be replayed from
# `balances` itself: it is a ReplacingMergeTree, whose merges keep only an account's latest row.
#   1. CREATE balances_window (ReplicatedMergeTree) and the view mv_balances_window TO it, fed
#      with every balances row that has a block_hash: the sink's rows (migration rows have none)
#   2. D = max(block_num) of balances: rows up to D were seen by the historical view   [state file]
#   3. DETACH TABLE mv_historical_balances_state ON CLUSTER ... PERMANENTLY (survives a restart)
#   4. one INSERT ... SELECT per two-character base58 account prefix [p, p~)          [progress file]
#   5. ATTACH TABLE mv_historical_balances_state ON CLUSTER, then DROP the capture view
#   6. replay: INSERT INTO historical_balances_state the view's own query over balances_window
#      WHERE block_num > D, then DROP balances_window
# mv_balances_by_account stays attached: every zero row reaches balances_by_account as the
# sink's rows do.
#
# Exactness: no sink row is missed. The capture view exists from before D is taken until after
# the historical view is attached again, and D excludes what the historical view already saw.
# The sink's inserts in flight at the detach, and those started between the attach and the drop
# of the capture view (two consecutive statements), are aggregated twice: open/high/low/close are
# argMin/argMax/min/max and unaffected, only `transactions` (a sum) is over by one for those
# rows. The same holds for the whole window if step 6 is rerun after a failure.
#
# Resume: Ctrl-C at any point, then rerun the same command; the state file says where it was.
# D is kept, the historical view is detached again if someone attached it, finished chunks are
# skipped, an interrupted chunk is redone (its duplicate rows collapse on merge). FINISH=1 skips
# the remaining chunks and runs steps 5-6 now, to restore the view early; a later plain run then
# starts a new cycle (steps 1-6 for the remaining chunks).
#
# Usage:
#   CH_URL=http://ch-node892g.riv.eosn.io:8123 CH_USER=<admin> CH_PASSWORD=<pw> ./migrate-v0.3.5-closed-accounts.sh
#   ONLY_PREFIX=Ab ...   # steps 1-4 for one chunk, to time it; the view stays detached:
#                        # continue with a plain run, or FINISH=1 to attach and replay now
#   FINISH=1 ...         # steps 5-6 only
# The user needs SELECT on both databases, INSERT on balances and historical_balances_state, and
# CREATE and DROP on the balances database (capture table and view, DETACH and ATTACH).
# Files, in the current directory: progress.txt (one line per finished chunk) and state.txt
# (D, ATTACHED, CAPTURE_DROPPED, REPLAY of the current cycle).
set -euo pipefail
: "${CH_URL:?}" "${CH_USER:?}" "${CH_PASSWORD:?}"
BAL_DB='solana:svm-balances@v0.3.3'
ACC_DB='solana:svm-accounts@v0.3.1'
BAL="\`$BAL_DB\`"
ACC="\`$ACC_DB\`"
MV=mv_historical_balances_state
WIN=balances_window
WIN_MV=mv_balances_window
CLUSTER=${CLUSTER-tokenapis-b}            # CLUSTER= (empty) omits ON CLUSTER, for a local test
ON_CLUSTER=""
if [ -n "$CLUSTER" ]; then ON_CLUSTER="ON CLUSTER '$CLUSTER'"; fi
WIN_ENGINE=${WIN_ENGINE-"ReplicatedMergeTree('/clickhouse/tables/{uuid}/{shard}', '{replica}')"}  # WIN_ENGINE=MergeTree for a local test
ALPHABET='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
PROGRESS=${PROGRESS:-progress.txt}
STATE=${STATE:-state.txt}
PARALLEL=${PARALLEL:-4}                  # chunks inserted at the same time
# Per-query settings. The 2026-11-02 profile of one chunk on prod (38 s): writing one 1 M-row
# block in a single sink thread took 14 s, the copy into balances_by_account ran serially for
# 7 s, and the on-insert dedup of the ReplacingMergeTree burnt 6 s on rows that have no
# duplicates (one row per (account, mint) per chunk). Hence parallel view processing and no
# on-insert dedup. Insert blocks stay at 1 M rows: with 200 K-row blocks (7 parts per chunk per
# table) at 50 chunks a minute, active parts of balances grew by about 200 a minute on prod
# (2026-11-02 21:33 UTC), heading for parts_to_delay_insert = 1000, where ClickHouse delays
# every insert into the table, the live sink's included.
SETTINGS=${SETTINGS:-'max_threads=8&max_insert_threads=4&min_insert_block_size_rows=1000000&min_insert_block_size_bytes=300000000&parallel_view_processing=1&optimize_on_insert=0&max_execution_time=1800&max_memory_usage=20000000000&distributed_ddl_task_timeout=600'}
touch "$PROGRESS" "$STATE"

say() { echo "$(date -u +%FT%TZ) $*"; }
ch() {  # $1 = query; prints the result (TSV); on a ClickHouse error prints the server's message and fails
  local out
  if ! out=$(curl -sS --fail-with-body --max-time 1900 -u "$CH_USER:$CH_PASSWORD" "$CH_URL/?$SETTINGS" --data-binary "$1" 2>&1); then
    say "ClickHouse error: $out" >&2
    say "query was: $(echo "$1" | tr -s ' \n' ' ' | cut -c1-300)" >&2
    return 1
  fi
  printf '%s' "$out"
}
state_get() { sed -n "s/^$1=//p" "$STATE" | tail -1; }
state_set() { echo "$1=$2" >> "$STATE"; }
exists() {  # $1 = table name in the balances database
  local n
  n=$(ch "SELECT count() FROM system.tables WHERE database = '$BAL_DB' AND name = '$1'") || exit 1
  [ "$n" = 1 ]
}
chunks_done() { cut -d' ' -f1 "$PROGRESS" | grep -E '^[1-9A-HJ-NP-Za-km-z]{2}$' | sort -u | wc -l | tr -d ' '; }
new_cycle() { mv "$STATE" "$STATE.$(date -u +%Y%m%dT%H%M%SZ)"; touch "$STATE"; }

create_capture() {
  ch "CREATE TABLE IF NOT EXISTS $BAL.$WIN $ON_CLUSTER
      (block_num UInt32, block_hash String, timestamp DateTime('UTC'), program_id LowCardinality(String),
       mint LowCardinality(String), account String, amount UInt64, decimals UInt8)
      ENGINE = $WIN_ENGINE ORDER BY block_num" > /dev/null
  ch "CREATE MATERIALIZED VIEW IF NOT EXISTS $BAL.$WIN_MV $ON_CLUSTER TO $BAL.$WIN AS
      SELECT block_num, block_hash, timestamp, program_id, mint, account, amount, decimals
      FROM $BAL.balances WHERE block_hash != ''" > /dev/null
  say "capture view $WIN_MV -> $WIN in place"
}

run_chunk() {  # $1 = two-character prefix
  local p=$1 t0 summary hdr body
  t0=$(date +%s)
  hdr=$(mktemp); body=$(mktemp)
  if ! curl -sS --fail-with-body --max-time 1900 -u "$CH_USER:$CH_PASSWORD" -D "$hdr" -o "$body" \
    "$CH_URL/?$SETTINGS" --data-binary "
INSERT INTO $BAL.balances (block_num, timestamp, program_id, mint, account, amount, decimals)
SELECT c.block_num, c.timestamp, b.program_id, b.mint, b.account, 0, b.decimals
FROM (
    SELECT account, program_id, mint,
           argMax(amount, block_num) AS amount, max(block_num) AS last_block, argMax(decimals, block_num) AS decimals
    FROM $BAL.balances_by_account
    WHERE account >= '$p' AND account < '$p~'
    GROUP BY account, program_id, mint
    HAVING amount > 0
) AS b
INNER JOIN (
    SELECT account, argMax(closed, version) AS closed, argMax(block_num, version) AS block_num, argMax(timestamp, version) AS timestamp
    FROM $ACC.close_account_state
    WHERE account >= '$p' AND account < '$p~'
    GROUP BY account
    HAVING closed = 1
) AS c USING (account)
WHERE c.block_num > b.last_block"; then
    say "chunk $p failed: $(cat "$body")" >&2
    rm -f "$hdr" "$body"
    exit 255   # under xargs, 255 stops new chunks from starting; the running ones finish
  fi
  summary=$(tr -d '\r' < "$hdr" | grep -i '^X-ClickHouse-Summary:' | tail -1)
  rm -f "$hdr" "$body"
  echo "$p $(date -u +%FT%TZ) $(( $(date +%s) - t0 ))s ${summary#*: }" | tee -a "$PROGRESS"
}

replay() {  # $1 = D: the historical view's own query over the captured rows after D
  ch "
INSERT INTO $BAL.historical_balances_state
    (interval_min, timestamp, min_block_num, max_block_num, program_id, mint, account, open, high, low, close, transactions)
WITH [1, 5, 10, 30, 60, 240, 1440, 10080] AS intervals
SELECT
    arrayJoin(intervals) AS interval_min,
    toDateTime((intDiv(toUInt32(b.timestamp), interval_min * 60) * interval_min) * 60) AS timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,
    program_id, mint, account,
    argMinState(amount, b.block_num) AS open,
    max(amount) AS high,
    min(amount) AS low,
    argMaxState(amount, b.block_num) AS close,
    count() AS transactions
FROM $BAL.$WIN AS b
WHERE block_num > $1
GROUP BY interval_min, account, program_id, mint, timestamp"
}

finish() {  # steps 5 and 6, resumable
  local D
  D=$(state_get D)
  if [ -z "$(state_get ATTACHED)" ]; then
    if exists "$MV"; then say "$MV is attached"; else ch "ATTACH TABLE $BAL.$MV $ON_CLUSTER" > /dev/null; say "attached $MV"; fi
    state_set ATTACHED 1
  fi
  if [ -z "$(state_get CAPTURE_DROPPED)" ]; then
    ch "DROP TABLE IF EXISTS $BAL.$WIN_MV $ON_CLUSTER SYNC" > /dev/null
    state_set CAPTURE_DROPPED 1
    say "dropped the capture view $WIN_MV"
  fi
  local n
  n=$(ch "SELECT count() FROM $BAL.$WIN WHERE block_num > $D")
  say "replaying the captured rows after block $D into historical_balances_state ($n rows)"
  replay "$D"
  ch "DROP TABLE IF EXISTS $BAL.$WIN $ON_CLUSTER SYNC" > /dev/null
  state_set REPLAY done
  say "done: $(chunks_done) of 3364 chunks in $PROGRESS, $MV attached, window after block $D replayed, $WIN dropped"
}

trap 'echo; say "interrupted; rerun the same command to continue from $STATE, or FINISH=1 to attach and replay now" >&2; exit 130' INT TERM

if [ "$(state_get REPLAY)" = done ]; then
  if [ -n "${FINISH:-}" ]; then say "this cycle is already finished (REPLAY=done in $STATE)"; exit 0; fi
  if [ "$(chunks_done)" -ge 3364 ]; then say "all 3364 chunks are in $PROGRESS and the last cycle is finished: nothing to do"; exit 0; fi
  new_cycle
  say "previous cycle finished; starting a new one for the $(( 3364 - $(chunks_done) )) remaining chunks"
fi

if [ -n "$(state_get ATTACHED)" ]; then finish; exit 0; fi

if [ -n "${FINISH:-}" ]; then
  [ -n "$(state_get D)" ] || { say "FINISH=1 but $STATE has no D: nothing to finish" >&2; exit 1; }
  say "FINISH=1: skipping the chunks, $(chunks_done) of 3364 done"
  finish; exit 0
fi

# 1-2. capture view, then the boundary
D=$(state_get D)
if [ -n "$D" ] && ! exists "$WIN_MV"; then
  if exists "$MV"; then
    say "D=$D is recorded but the capture view is gone while $MV is attached: nothing was missed, starting the cycle over"
    new_cycle; D=
  else
    say "D=$D is recorded, $MV is detached and the capture view $WIN_MV is gone: the sink's rows since cannot be replayed. Run ATTACH TABLE $BAL.$MV $ON_CLUSTER and investigate before rerunning." >&2
    exit 1
  fi
fi
if [ -z "$D" ]; then
  if exists "$WIN_MV"; then
    say "$STATE has no D but the capture view $WIN_MV already exists: a cycle is in progress from another directory. Run from that directory (or point STATE= and PROGRESS= at its files); starting a new cycle here would lose captured rows." >&2
    exit 1
  fi
  create_capture
  D=$(ch "SELECT max(block_num) FROM $BAL.balances")
  state_set D "$D"
  say "D=$D recorded (max block_num of balances before the detach)"
else
  say "D=$D from $STATE, capture view present"
fi
# 3. detach the historical view
if exists "$MV"; then
  ch "DETACH TABLE $BAL.$MV $ON_CLUSTER PERMANENTLY" > /dev/null
  say "detached $MV"
else
  say "$MV is already detached"
fi
# 4. chunks
if [ -n "${ONLY_PREFIX:-}" ]; then
  run_chunk "$ONLY_PREFIX"
  say "one chunk done; $MV is detached and $WIN_MV is capturing: continue with a plain run, or FINISH=1 to attach and replay now"
  exit 0
fi
say "$(chunks_done) of 3364 chunks already in $PROGRESS; running $PARALLEL at a time"
pending() {
  for a in $(echo "$ALPHABET" | fold -w1); do
    for b in $(echo "$ALPHABET" | fold -w1); do
      grep -q "^$a$b " "$PROGRESS" || echo "$a$b"
    done
  done
}
export -f run_chunk say
export CH_URL CH_USER CH_PASSWORD SETTINGS BAL ACC PROGRESS
pending | xargs -P "$PARALLEL" -n 1 bash -c 'run_chunk "$1"' _ || true
if [ "$(chunks_done)" -lt 3364 ]; then
  say "$(( 3364 - $(chunks_done) )) chunks are not in $PROGRESS (failed or interrupted): rerun the same command to retry them; the view stays detached meanwhile" >&2
  exit 1
fi
say "all 3364 chunks are in $PROGRESS"
# 5-6
finish
