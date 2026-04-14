CREATE MATERIALIZED VIEW IF NOT EXISTS metadata
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY (mint)
AS
WITH metadata_mint AS (
    SELECT mint, argMax(metadata, version) AS metadata
    FROM metadata_mint_state
    GROUP BY mint
),
metadata_name AS (
    SELECT metadata, name
    FROM metadata_name_state FINAL
    WHERE metadata IN (SELECT metadata FROM metadata_mint)
),
metadata_symbol AS (
    SELECT metadata, symbol
    FROM metadata_symbol_state FINAL
    WHERE metadata IN (SELECT metadata FROM metadata_mint)
),
metadata_uri AS (
    SELECT metadata, uri
    FROM metadata_uri_state FINAL
    WHERE metadata IN (SELECT metadata FROM metadata_mint)
)
SELECT
    mm.mint,
    mm.metadata as metadata,
    n.name,
    s.symbol,
    u.uri
FROM metadata_mint AS mm
LEFT JOIN metadata_name AS n ON mm.metadata = n.metadata
LEFT JOIN metadata_symbol AS s ON mm.metadata = s.metadata
LEFT JOIN metadata_uri AS u ON mm.metadata = u.metadata
SETTINGS max_threads = 4;