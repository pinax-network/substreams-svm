-- Pool activity (Transactions) --
CREATE TABLE IF NOT EXISTS pool_activity_summary (
    -- Order By --
    program_id                  LowCardinality(String),
    amm                         LowCardinality(String),
    amm_pool                    LowCardinality(String),
    mint0                       LowCardinality(String),
    mint1                       LowCardinality(String),

    -- summing --
    transactions                UInt64,

    -- indexes --
    INDEX idx_program_id        (program_id)            TYPE set(8)                 GRANULARITY 1,
    INDEX idx_amm               (amm)                   TYPE set(256)               GRANULARITY 1,
    INDEX idx_amm_pool          (amm_pool)              TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_mint0             (mint0)                 TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_mint1             (mint1)                 TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_mint_pair         (mint0, mint1)          TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_transactions      (transactions)          TYPE minmax                 GRANULARITY 1
)
ENGINE = SummingMergeTree
ORDER BY (program_id, amm, amm_pool, mint0, mint1)
COMMENT 'Summary of pool activity (transactions) for AMM pools';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_pool_activity_summary
TO pool_activity_summary
AS
WITH
    (input_mint <= output_mint) AS dir,
    if (dir, input_mint,  output_mint) AS mint0,
    if (dir, output_mint, input_mint) AS mint1
SELECT
    program_id,
    amm,
    amm_pool,
    mint0,
    mint1,

    -- summing --
    1 as transactions
FROM swaps;
