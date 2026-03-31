-- SPL Token Balances --
CREATE TABLE IF NOT EXISTS balances (
    -- block --
    block_num       UInt32,
    block_hash      String,
    timestamp       DateTime(0, 'UTC'),

    -- balance --
    program_id      LowCardinality(String),
    mint            LowCardinality(String),
    account         String,
    amount          UInt64,
    decimals        UInt8,

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

-- Native Token Balances --
CREATE TABLE IF NOT EXISTS native_balances (
    -- block --
    block_num       UInt32,
    block_hash      String,
    timestamp       DateTime(0, 'UTC'),

    -- balance --
    account         String,
    amount          UInt64,

    -- indexes --
    INDEX idx_amount (amount) TYPE minmax GRANULARITY 1
)
ENGINE = ReplacingMergeTree(block_num)
ORDER BY (account)
COMMENT 'Native SOL balances (single balance per-block per-account)';