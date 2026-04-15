-- Native SOL holders + circulating supply summary
CREATE MATERIALIZED VIEW IF NOT EXISTS native_balances_metadata
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY tuple()
AS
SELECT
    count() AS holders,
    sum(amount) AS circulating_supply,
    max(block_num) AS block_num,
    max(timestamp) AS timestamp
FROM native_balances FINAL
WHERE amount > 0;

-- -- Optional: initialize immediately
-- SYSTEM REFRESH VIEW native_balances_metadata;
