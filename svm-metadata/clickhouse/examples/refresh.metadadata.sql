-- inspect progress --
SELECT
    mutation_id,
    command,
    parts_to_do,
    is_done
FROM system.mutations
WHERE `table` = 'metadata_mint_state'
ORDER BY create_time DESC

-- confirm the projection exists --
SELECT database, table, name
FROM system.projections
WHERE table = 'metadata_mint_state';

-- refreshable --
SYSTEM REFRESH VIEW metadata;

-- inspect progress --
SELECT *
FROM system.view_refreshes
WHERE view = 'metadata';