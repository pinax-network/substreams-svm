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

    -- count() --
    PROJECTION prj_mint_count ( SELECT program_id, mint, min(amount), max(amount), count(), max(block_num), min(block_num), max(timestamp), min(timestamp) GROUP BY program_id, mint ),

    -- projections --
    PROJECTION prj_account_mint ( SELECT * ORDER BY account, program_id, mint )
)
ENGINE = ReplacingMergeTree(block_num)
ORDER BY (mint, account)
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

    -- count() --
    PROJECTION prj_count ( SELECT min(amount), max(amount), count(), max(block_num), min(block_num), max(timestamp), min(timestamp) ),
)
ENGINE = ReplacingMergeTree(block_num)
ORDER BY (account)
COMMENT 'Native SOL balances (single balance per-block per-account)';