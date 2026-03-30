-- OHLCV prices --
CREATE TABLE IF NOT EXISTS ohlc_prices (
    timestamp               DateTime('UTC', 0) COMMENT 'beginning of the bar',

    -- OrderBy --
    program_id              LowCardinality(String),
    amm                     LowCardinality(String),
    amm_pool                LowCardinality(String),
    mint0                   LowCardinality(String),
    mint1                   LowCardinality(String),

    -- Aggregate --
    open0                   AggregateFunction(argMin, Float64, UInt64),
    quantile0               AggregateFunction(quantileDeterministic, Float64, UInt64),
    close0                  AggregateFunction(argMax, Float64, UInt64),

    -- volume --
    gross_volume0           SimpleAggregateFunction(sum, Int128) COMMENT 'gross volume of token0 in the window',
    gross_volume1           SimpleAggregateFunction(sum, Int128) COMMENT 'gross volume of token1 in the window',
    net_flow0               SimpleAggregateFunction(sum, Int128) COMMENT 'net flow of token0 in the window',
    net_flow1               SimpleAggregateFunction(sum, Int128) COMMENT 'net flow of token1 in the window',

    -- universal --
    uaw                     AggregateFunction(uniq, String) COMMENT 'unique wallet addresses in the window',
    transactions            SimpleAggregateFunction(sum, UInt64) COMMENT 'number of transactions in the window',

    -- indexes --
    INDEX idx_timestamp         (timestamp)         TYPE minmax                 GRANULARITY 1,
    INDEX idx_program_id        (program_id)        TYPE set(8)                 GRANULARITY 1,
    INDEX idx_amm               (amm)               TYPE set(256)               GRANULARITY 1,
    INDEX idx_amm_pool          (amm_pool)          TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_mint0             (mint0)             TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_mint1             (mint1)             TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_mint_pair         (mint0, mint1)      TYPE bloom_filter(0.005)    GRANULARITY 1,

    -- indexes (volume) --
    INDEX idx_gross_volume0     (gross_volume0)     TYPE minmax         GRANULARITY 1,
    INDEX idx_gross_volume1     (gross_volume1)     TYPE minmax         GRANULARITY 1,
    INDEX idx_net_flow0         (net_flow0)         TYPE minmax         GRANULARITY 1,
    INDEX idx_net_flow1         (net_flow1)         TYPE minmax         GRANULARITY 1,
    INDEX idx_transactions      (transactions)      TYPE minmax         GRANULARITY 1,
)
ENGINE = AggregatingMergeTree
ORDER BY (timestamp, program_id, amm, amm_pool, mint0, mint1)
COMMENT 'OHLCV prices for AMM pools, aggregated by hour';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_ohlc_prices
TO ohlc_prices
AS
WITH
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
    if(dir, -toInt128(output_amount), toInt128(input_amount))  AS nf1,
    -- version is used to determine min/max states --
    toUInt64(to_version(block_num, transaction_index, instruction_index)) AS version

SELECT
    toStartOfHour(swaps.timestamp)    AS timestamp,
    program_id, amm, amm_pool, mint0, mint1,

    /* OHLC */
    argMinState(price, version)                 AS open0,
    quantileDeterministicState(price, version)  AS quantile0,
    argMaxState(price, version)                 AS close0,

    /* volumes & flows (all in canonical orientation) */
    sum(gv0)                AS gross_volume0,
    sum(gv1)                AS gross_volume1,
    sum(nf0)                AS net_flow0,
    sum(nf1)                AS net_flow1,

    /* universal */
    uniqState(user)         AS uaw,
    count()                 AS transactions
FROM swaps
GROUP BY timestamp, program_id, amm, amm_pool, mint0, mint1;

