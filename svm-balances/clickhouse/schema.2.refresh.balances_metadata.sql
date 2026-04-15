-- SPL/SPL-2022 holders + circulating supply summary per mint
CREATE MATERIALIZED VIEW IF NOT EXISTS balances_metadata
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY (program_id, mint)
AS
SELECT
    program_id,
    mint,
    count() AS holders,
    sum(amount) AS circulating_supply,
    max(block_num) AS block_num,
    max(timestamp) AS timestamp
FROM balances FINAL
WHERE amount > 0
GROUP BY program_id, mint;

-- -- Optional: initialize immediately
-- SYSTEM REFRESH VIEW balances_metadata;
