
-- SPL Token top holders: max 10,000 accounts per program_id/mint
CREATE MATERIALIZED VIEW balances_holders
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY (program_id, mint, amount DESC, account)
SETTINGS allow_experimental_reverse_key = 1
AS
SELECT
    block_num,
    timestamp,
    program_id,
    mint,
    account,
    amount,
    decimals
FROM balances FINAL
WHERE amount > 0
ORDER BY program_id, mint, amount DESC, account
LIMIT 10000 BY program_id, mint;

-- Native SOL top holders: max 100,000 accounts total
CREATE MATERIALIZED VIEW native_balances_holders
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY (amount DESC, account)
SETTINGS allow_experimental_reverse_key = 1
AS
SELECT
    block_num,
    timestamp,
    account,
    amount
FROM native_balances FINAL
WHERE amount > 0
ORDER BY amount DESC, account
LIMIT 100000;

-- -- Optional: initialize immediately
-- SYSTEM REFRESH VIEW balances_holders;
-- SYSTEM REFRESH VIEW native_balances_holders;
