-- SPL Token Balances --
CREATE TABLE IF NOT EXISTS balances (
    -- block --
    block_num       UInt32,
    block_hash      String,
    timestamp       DateTime(0, 'UTC'),

    -- balance --
    program_id      LowCardinality(String),
    account         String,
    amount          UInt64,
    mint            Nullable(String),
    decimals        Nullable(UInt8),

    -- indexes --
    INDEX idx_program_id (program_id) TYPE set(2) GRANULARITY 1,
    INDEX idx_amount (amount) TYPE minmax GRANULARITY 1,
    INDEX idx_decimals (decimals) TYPE minmax GRANULARITY 1,

    -- projections --
    PROJECTION prj_mint (SELECT * ORDER BY (mint, account))
)
ENGINE = ReplacingMergeTree(block_num)
ORDER BY (account)
SETTINGS deduplicate_merge_projection_mode = 'rebuild'
COMMENT 'SPL Token balances (single balance per-block per-account/mint)';


CREATE MATERIALIZED VIEW IF NOT EXISTS mv_post_token_balances
TO balances AS
SELECT
    block_num,
    block_hash,
    timestamp,
    program_id,
    account,
    amount,
    mint,
    decimals
FROM post_token_balances;
