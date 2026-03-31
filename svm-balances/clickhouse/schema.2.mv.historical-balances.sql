-- Historical balances by address/contract --
CREATE TABLE IF NOT EXISTS historical_balances_state (
    -- block --
    timestamp            DateTime(0, 'UTC') COMMENT 'the start of the aggregate window',
    interval_min         UInt16 DEFAULT 1 COMMENT 'bar interval in minutes (1m, 5m, 10m, 30m, 1h, 4h, 1d, 1w)',
    min_block_num        SimpleAggregateFunction(min, UInt32) COMMENT 'the minimum block number in the aggregate window',
    max_block_num        SimpleAggregateFunction(max, UInt32) COMMENT 'the maximum block number in the aggregate window',

    -- balance change --
    program_id           LowCardinality(String),
    mint                 LowCardinality(String),
    account              String,

    -- ohlc --
    open                 AggregateFunction(argMin, UInt64, UInt32),
    high                 SimpleAggregateFunction(max, UInt64),
    low                  SimpleAggregateFunction(min, UInt64),
    close                AggregateFunction(argMax, UInt64, UInt32),
    transactions         SimpleAggregateFunction(sum, UInt64) COMMENT 'total number of transactions in the window'
)
ENGINE = AggregatingMergeTree
ORDER BY (interval_min, account, program_id, mint, timestamp);

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_historical_balances_state
TO historical_balances_state
AS
WITH
    -- predefined intervals --
    -- in minutes: 1m, 5m, 10m, 30m, 1h, 4h, 1d, 1w
    [1, 5, 10, 30, 60, 240, 1440, 10080] AS intervals

SELECT
    arrayJoin(intervals) AS interval_min,
    -- floor to the interval in seconds
    toDateTime(intDiv(toUInt32(b.timestamp), interval_min * 60) * interval_min * 60) AS timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,

    -- balance change --
    program_id,
    mint,
    account,

    -- ohlc --
    argMinState(amount, b.block_num) AS open,
    max(amount) AS high,
    min(amount) AS low,
    argMaxState(amount, b.block_num) AS close,
    count() AS transactions
FROM balances AS b
GROUP BY interval_min, account, program_id, mint, timestamp;
