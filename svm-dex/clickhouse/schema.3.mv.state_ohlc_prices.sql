-- OHLCV prices --
CREATE TABLE IF NOT EXISTS state_ohlc_prices (
    -- bar interval --
    timestamp               DateTime('UTC', 0) COMMENT 'beginning of the bar',
    interval_min            UInt16 DEFAULT 1 COMMENT 'bar interval in minutes (1m, 5m, 10m, 30m, 1h, 4h, 1d, 1w)',

    -- timestamp & block number --
    min_timestamp           SimpleAggregateFunction(min, DateTime('UTC', 0)) COMMENT 'first timestamp seen',
    max_timestamp           SimpleAggregateFunction(max, DateTime('UTC', 0)) COMMENT 'last timestamp seen',
    min_block_num           SimpleAggregateFunction(min, UInt32) COMMENT 'first block number seen',
    max_block_num           SimpleAggregateFunction(max, UInt32) COMMENT 'last block number seen',

    -- DEX identity --
    protocol                    Enum8(
        'unspecified' = 0,
        'boop' = 1,
        'darklake' = 2,
        'dumpfun' = 3,
        'jupiter_v4' = 4,
        'jupiter_v6' = 5,
        'meteora_daam' = 6,
        'meteora_dlmm' = 7,
        'orca_whirlpool' = 8,
        'pumpfun' = 9,
        'pumpfun_amm' = 10,
        'raydium_amm_v4' = 11,
        'raydium_clmm' = 12,
        'raydium_cpmm' = 13,
        'raydium_launchpad' = 14
    ) COMMENT 'Protocol',
    program_id              LowCardinality(String),
    amm                     LowCardinality(String),
    amm_pool                LowCardinality(String),
    mint0                   LowCardinality(String),
    mint1                   LowCardinality(String),

    -- Aggregate --
    open0                   AggregateFunction(argMin, Float64, UInt64) COMMENT 'opening price of token0 in the window',
    quantile0               AggregateFunction(quantileDeterministic, Float64, UInt64) COMMENT 'quantile price of token0 in the window',
    close0                  AggregateFunction(argMax, Float64, UInt64) COMMENT 'closing price of token0 in the window',

    -- volume --
    gross_volume0           SimpleAggregateFunction(sum, Int128) COMMENT 'gross volume of token0 in the window',
    gross_volume1           SimpleAggregateFunction(sum, Int128) COMMENT 'gross volume of token1 in the window',
    net_flow0               SimpleAggregateFunction(sum, Int128) COMMENT 'net flow of token0 in the window',
    net_flow1               SimpleAggregateFunction(sum, Int128) COMMENT 'net flow of token1 in the window',

    -- universal --
    -- universal --
    transactions            SimpleAggregateFunction(sum, UInt64) COMMENT 'number of transactions in the window',
    uniq_signer             AggregateFunction(uniq, String) COMMENT 'unique transaction signer addresses in the window',
    uniq_fee_payer          AggregateFunction(uniq, String) COMMENT 'unique fee payer addresses in the window',
    uniq_user               AggregateFunction(uniq, String) COMMENT 'unique swap user addresses in the window',

    -- indexes (timestamps & blocks) --
    INDEX idx_timestamp         (timestamp)         TYPE minmax                 GRANULARITY 1,
    INDEX idx_min_timestamp     (min_timestamp)     TYPE minmax                 GRANULARITY 1,
    INDEX idx_max_timestamp     (max_timestamp)     TYPE minmax                 GRANULARITY 1,
    INDEX idx_min_block_num     (min_block_num)     TYPE minmax                 GRANULARITY 1,
    INDEX idx_max_block_num     (max_block_num)     TYPE minmax                 GRANULARITY 1,

    -- indexes (dimensions) --
    INDEX idx_program_id        (program_id)        TYPE set(16)                GRANULARITY 1,
    INDEX idx_amm               (amm)               TYPE set(256)               GRANULARITY 1,
    INDEX idx_amm_pool          (amm_pool)          TYPE bloom_filter           GRANULARITY 1,
    INDEX idx_mint0             (mint0)             TYPE bloom_filter           GRANULARITY 1,
    INDEX idx_mint1             (mint1)             TYPE bloom_filter           GRANULARITY 1,
    INDEX idx_mint_pair         (mint0, mint1)      TYPE bloom_filter           GRANULARITY 1,

    -- indexes (volume) --
    INDEX idx_gross_volume0     (gross_volume0)     TYPE minmax         GRANULARITY 1,
    INDEX idx_gross_volume1     (gross_volume1)     TYPE minmax         GRANULARITY 1,
    INDEX idx_net_flow0         (net_flow0)         TYPE minmax         GRANULARITY 1,
    INDEX idx_net_flow1         (net_flow1)         TYPE minmax         GRANULARITY 1,
    INDEX idx_transactions      (transactions)      TYPE minmax         GRANULARITY 1,
)
ENGINE = AggregatingMergeTree
ORDER BY (
    interval_min,
    amm_pool, program_id, amm, mint0, mint1,
    timestamp
)
COMMENT 'OHLCV prices for AMM pools, aggregated by interval';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_state_ohlc_prices
TO state_ohlc_prices
AS
WITH
    -- predefined intervals --
    -- in minutes: 1m, 5m, 10m, 30m, 1h, 4h, 1d, 1w
    [1, 5, 10, 30, 60, 240, 1440, 10080] AS intervals,

    -- canonical token ordering
    (input_mint <= output_mint) AS dir,
    if (dir, input_mint,  output_mint) AS mint0,
    if (dir, output_mint, input_mint) AS mint1,
    if (dir, input_amount,  output_amount) AS amount0,
    if (dir, output_amount, input_amount) AS amount1,
    toFloat64(amount1) / amount0 AS price,
    abs(amount0) AS gv0,
    abs(amount1) AS gv1,
    -- net flow of mint0: +in, -out
    if(dir, toInt128(input_amount), -toInt128(output_amount))  AS nf0,
    -- net flow of mint1: +in, -out (signs flipped vs. your original)
    if(dir, -toInt128(output_amount), toInt128(input_amount))  AS nf1

SELECT
    arrayJoin(intervals) AS interval_min,
    -- floor to the interval in seconds
    toDateTime(intDiv(toUInt32(s.timestamp), interval_min * 60) * interval_min * 60) AS timestamp,

    -- timestamp & block number --
    min(s.timestamp) AS min_timestamp,
    max(s.timestamp) AS max_timestamp,
    min(s.block_num) AS min_block_num,
    max(s.block_num) AS max_block_num,

    -- dimensions --
    program_id, amm, amm_pool, mint0, mint1,

    /* OHLC */
    argMinState(price, toUInt64(block_num))                 AS open0,
    quantileDeterministicState(price, toUInt64(block_num))  AS quantile0,
    argMaxState(price, toUInt64(block_num))                 AS close0,

    -- volumes & flows (all in canonical orientation) --
    sum(gv0)                AS gross_volume0,
    sum(gv1)                AS gross_volume1,
    sum(nf0)                AS net_flow0,
    sum(nf1)                AS net_flow1,

    -- universal --
    uniqState(signer)       AS uniq_signer,
    uniqState(fee_payer)    AS uniq_fee_payer,
    uniqState(user)         AS uniq_user,
    count()                 AS transactions
FROM swaps s
GROUP BY
    -- bar interval
    interval_min,
    -- canonical token ordering
    amm_pool, protocol, program_id, amm, mint0, mint1,
     -- bar beginning
    timestamp;
