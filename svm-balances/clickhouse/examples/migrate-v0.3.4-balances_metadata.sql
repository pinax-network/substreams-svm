-- Create the refresh MV on the live cluster.
CREATE MATERIALIZED VIEW IF NOT EXISTS balances_metadata ON CLUSTER 'tokenapis-b'
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

-- Trigger the first refresh locally on each node where needed.
SYSTEM REFRESH VIEW balances_metadata;

-- Check refresh progress and latest status.
SELECT
    database,
    view,
    status,
    last_refresh_time,
    last_success_time,
    last_success_duration_ms,
    progress,
    read_rows,
    read_bytes,
    total_rows,
    written_rows,
    written_bytes,
    exception
FROM system.view_refreshes
WHERE view = 'balances_metadata';

-- Query the compact result.
SELECT
    program_id,
    mint,
    holders,
    circulating_supply,
    block_num,
    timestamp
FROM balances_metadata
ORDER BY program_id, mint
LIMIT 100;
