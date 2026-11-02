#!/usr/bin/env bash
# Second pass of the svm-balances v0.3.5 migration: accounts closed in the SAME block as their
# last positive balance row. The first pass (migrate-v0.3.5-closed-accounts.sh) used
# close_block > last_balance_block and so skipped them; wSOL temporary accounts are typically
# funded and closed within one block, and 848 of wSOL's top 1,000 holders were still closed
# accounts after the first pass (2026-11-02 22:45 UTC), all of this shape.
#
# A zero row is inserted with the close's block_num and timestamp, equal to the stale row's.
# In both ReplacingMergeTrees the later-inserted row wins among equal versions, so the zero
# row replaces the stale one in balances and marks the account deleted in balances_by_account.
# Safe because the account's latest svm-accounts state is closed with no later open, so the
# close is the last event of that block. Measured on chunk Ab: 15,719 rows against 1.29 M in
# the first pass, about 15 M rows in total, so the historical view stays attached (about
# 120 M historical rows) and no capture/replay is needed. Idempotent and resumable like the
# first pass: progress-same-block.txt in the current directory, PARALLEL chunks at a time.
#
# Usage: CH_URL=http://ch-node892g.riv.eosn.io:8123 CH_USER=<admin> CH_PASSWORD=<pw> ./migrate-v0.3.5-same-block.sh
#        ONLY_PREFIX=Ab ...   # one chunk, to time it
set -euo pipefail
: "${CH_URL:?}" "${CH_USER:?}" "${CH_PASSWORD:?}"
BAL='`solana:svm-balances@v0.3.3`'
ACC='`solana:svm-accounts@v0.3.1`'
ALPHABET='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
PROGRESS=${PROGRESS:-progress-same-block.txt}
PARALLEL=${PARALLEL:-4}
SETTINGS=${SETTINGS:-'max_threads=8&max_insert_threads=4&min_insert_block_size_rows=1000000&min_insert_block_size_bytes=300000000&parallel_view_processing=1&optimize_on_insert=0&max_execution_time=1800&max_memory_usage=20000000000'}
touch "$PROGRESS"

say() { echo "$(date -u +%FT%TZ) $*"; }
chunks_done() { cut -d' ' -f1 "$PROGRESS" | grep -E '^[1-9A-HJ-NP-Za-km-z]{2}$' | sort -u | wc -l | tr -d ' '; }

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
WHERE c.block_num = b.last_block"; then
    say "chunk $p failed: $(cat "$body")" >&2
    rm -f "$hdr" "$body"
    exit 255
  fi
  summary=$(tr -d '\r' < "$hdr" | grep -i '^X-ClickHouse-Summary:' | tail -1)
  rm -f "$hdr" "$body"
  echo "$p $(date -u +%FT%TZ) $(( $(date +%s) - t0 ))s ${summary#*: }" | tee -a "$PROGRESS"
}

trap 'echo; say "interrupted; rerun the same command to continue from $PROGRESS" >&2; exit 130' INT TERM

if [ -n "${ONLY_PREFIX:-}" ]; then run_chunk "$ONLY_PREFIX"; exit 0; fi
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
  say "$(( 3364 - $(chunks_done) )) chunks are not in $PROGRESS (failed or interrupted): rerun the same command to retry them" >&2
  exit 1
fi
say "done: all 3364 chunks in $PROGRESS"
