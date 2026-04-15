-- SPL Token Balances (account-first lookup path) --
CREATE TABLE IF NOT EXISTS balances_by_account AS balances
ENGINE = ReplacingMergeTree(block_num, is_deleted)
ORDER BY (account, program_id, mint)
SETTINGS deduplicate_merge_projection_mode = 'rebuild'
COMMENT 'SPL Token balances optimized for per-account balance lookups';

ALTER TABLE balances_by_account DROP PROJECTION IF EXISTS prj_mint_count;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_balances_by_account
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