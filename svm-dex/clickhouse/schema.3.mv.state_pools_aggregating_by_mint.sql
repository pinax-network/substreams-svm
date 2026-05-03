-- Pool activity (Transactions) --
CREATE TABLE IF NOT EXISTS state_pools_aggregating_by_mint (
    -- timestamp & block number --
    min_timestamp         SimpleAggregateFunction(min, DateTime('UTC', 0)) COMMENT 'first timestamp seen',
    max_timestamp         SimpleAggregateFunction(max, DateTime('UTC', 0)) COMMENT 'last timestamp seen',
    min_block_num         SimpleAggregateFunction(min, UInt32) COMMENT 'first block number seen',
    max_block_num         SimpleAggregateFunction(max, UInt32) COMMENT 'last block number seen',

    -- DEX identity
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
        'raydium_launchpad' = 14,
        'meteora_amm' = 15,
        'byreal' = 16,
        'moonshot' = 17,
        'pancakeswap' = 18
    ) COMMENT 'Protocol',
    program_id              LowCardinality(String),
    amm                     LowCardinality(String),
    amm_pool                LowCardinality(String),
    mint                    String,

    -- universal --
    transactions            SimpleAggregateFunction(sum, UInt64) COMMENT 'total number of transactions',

    -- indexes --
    INDEX idx_min_timestamp     (min_timestamp)              TYPE minmax             GRANULARITY 1,
    INDEX idx_max_timestamp     (max_timestamp)              TYPE minmax             GRANULARITY 1,
    INDEX idx_min_block_num     (min_block_num)              TYPE minmax             GRANULARITY 1,
    INDEX idx_max_block_num     (max_block_num)              TYPE minmax             GRANULARITY 1,
    INDEX idx_protocol          (protocol)                   TYPE set(8)             GRANULARITY 1,
    INDEX idx_program_id        (program_id)                 TYPE set(1024)          GRANULARITY 1,
    INDEX idx_amm               (amm)                        TYPE set(1024)          GRANULARITY 1,
    INDEX idx_amm_pool          (amm_pool)                   TYPE set(1024)          GRANULARITY 1,
    INDEX idx_transactions      (transactions)               TYPE minmax             GRANULARITY 1,

    -- projections --
    -- optimize for distinct mints by pool --
    PROJECTION prj_mints_by_pool ( SELECT amm_pool, mint GROUP BY amm_pool, mint ),
    -- optimize for universal summary & distinct mints --
    PROJECTION prj_group_by_pool (
        SELECT
            -- timestamp & block number --
            min(min_timestamp),
            max(max_timestamp),
            min(min_block_num),
            max(max_block_num),

            -- DEX identity --
            protocol,
            program_id,
            amm,
            amm_pool,
            arraySort(groupArrayDistinct(mint)),

            -- universal --
            sum(transactions)
        GROUP BY protocol, program_id, amm, amm_pool
    )
)
ENGINE = AggregatingMergeTree
ORDER BY (mint, protocol, program_id, amm, amm_pool)
SETTINGS deduplicate_merge_projection_mode = 'rebuild'
COMMENT 'Aggregating pools by mint optimize for universal summary & distinct mints';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_state_pools_aggregating_by_mint_input_mint
TO state_pools_aggregating_by_mint
AS
SELECT
    -- timestamp & block number --
    min(timestamp) AS min_timestamp,
    max(timestamp) AS max_timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,

    -- DEX identity
    protocol, program_id, amm, amm_pool,
    input_mint AS mint,

    -- universal --
    count() as transactions
FROM swaps
GROUP BY mint, protocol, program_id, amm, amm_pool;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_state_pools_aggregating_by_mint_output_mint
TO state_pools_aggregating_by_mint
AS
SELECT
    -- timestamp & block number --
    min(timestamp) AS min_timestamp,
    max(timestamp) AS max_timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,

    -- DEX identity
    protocol, program_id, amm, amm_pool,
    output_mint AS mint,

    -- universal --
    count() as transactions
FROM swaps
GROUP BY mint, protocol, program_id, amm, amm_pool;
