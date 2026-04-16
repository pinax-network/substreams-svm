-- SPL Token Balances --
CREATE TABLE IF NOT EXISTS balances_by_account ON CLUSTER 'tokenapis-b' (
    -- block --
    block_num       UInt32,
    timestamp       DateTime(0, 'UTC'),

    -- balance --
    program_id      Enum8(
                        'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' = 1,
                        'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb' = 2
                    ),
    mint            LowCardinality(String),
    account         String,
    amount          UInt64,
    is_deleted      UInt8 MATERIALIZED amount = 0,
    decimals        UInt8,

    -- indexes --
    INDEX idx_program_id (program_id) TYPE set(2) GRANULARITY 1,
    INDEX idx_amount (amount) TYPE minmax GRANULARITY 1
)
ENGINE = ReplicatedReplacingMergeTree(block_num, is_deleted)
ORDER BY (account, program_id, mint)
COMMENT 'SPL Token balances (single balance per-block per-account/mint)';

-- Load new data into balances_by_account from balances --
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_balances_by_account ON CLUSTER 'tokenapis-b'
TO balances_by_account AS
SELECT
    block_num,
    timestamp,
    program_id,
    mint,
    account,
    amount,
    decimals
FROM balances;

-- one time backfill for existing data --
INSERT INTO balances_by_account
SELECT
    block_num,
    timestamp,
    program_id,
    mint,
    account,
    amount,
    decimals
FROM balances;

-- Detach and re-attach the materialized view to ensure it picks up the new data
DETACH TABLE `solana:svm-balances@v0.3.3`.mv_balances_by_account
ON CLUSTER `tokenapis-b`;

-- insert after last detach
INSERT INTO balances_by_account
SELECT
    block_num,
    timestamp,
    program_id,
    mint,
    account,
    amount,
    decimals
FROM balances
WHERE block_num > (SELECT max(block_num) FROM balances_by_account);

-- ATTACH the materialized view again to ensure it's active and picking up new data
ATTACH TABLE `solana:svm-balances@v0.3.3`.mv_balances_by_account
ON CLUSTER `tokenapis-b`;