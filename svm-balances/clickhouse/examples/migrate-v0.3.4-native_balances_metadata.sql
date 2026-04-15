-- Create the refresh MV on the live cluster.
CREATE MATERIALIZED VIEW IF NOT EXISTS native_balances_metadata ON CLUSTER 'tokenapis-b'
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

-- Trigger the first refresh locally on each node where needed.
SYSTEM REFRESH VIEW native_balances_metadata;

-- Check refresh progress and latest status.
SELECT *
FROM system.view_refreshes
WHERE view = 'native_balances_metadata';

-- Query the compact result.
SELECT
    holders,
    circulating_supply,
    block_num,
    timestamp
FROM native_balances_metadata;
