-- Add and backfill the projection used by:
-- SELECT DISTINCT mint
-- FROM state_pools_aggregating_by_mint
-- WHERE amm_pool = '...'

ALTER TABLE state_pools_aggregating_by_mint
ADD PROJECTION IF NOT EXISTS prj_mints_by_pool
(
    SELECT
        amm_pool,
        mint
    GROUP BY amm_pool, mint
);

ALTER TABLE state_pools_aggregating_by_mint
MATERIALIZE PROJECTION prj_mints_by_pool;

-- Check active / recent projection materialization mutations.
SELECT
    database,
    table,
    mutation_id,
    command,
    parts_to_do,
    is_done,
    latest_fail_reason
FROM system.mutations
WHERE table = 'state_pools_aggregating_by_mint'
ORDER BY create_time DESC;

-- Check whether projection parts have been built.
SELECT
    database,
    table,
    parent_name,
    name,
    active,
    count() AS parts
FROM system.projection_parts
WHERE table = 'state_pools_aggregating_by_mint'
GROUP BY database, table, parent_name, name, active
ORDER BY name, active DESC;

-- Confirm the optimizer can use the projection for pool -> mint lookup.
EXPLAIN indexes = 1
SELECT DISTINCT mint
FROM state_pools_aggregating_by_mint
WHERE amm_pool = 'AmmpSnW5xVeKHTAU9fMjyKEMPgrzmUj3ah5vgvHhAB5J';
