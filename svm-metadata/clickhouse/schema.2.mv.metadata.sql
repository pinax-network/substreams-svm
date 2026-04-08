-- TEMPLATE FOR MINT TABLES
CREATE TABLE IF NOT EXISTS TEMPLATE_METADATA (
    metadata     String,
    program_id   LowCardinality(String),
    version      UInt64,
    is_deleted   UInt8,
    block_num    UInt32,
    timestamp    DateTime(0, 'UTC'),

    -- indexes --
    INDEX idx_block_num (block_num) TYPE minmax GRANULARITY 1,
    INDEX idx_timestamp (timestamp) TYPE minmax GRANULARITY 1,
    INDEX idx_program_id (program_id) TYPE set(2) GRANULARITY 1,
    INDEX idx_is_deleted (is_deleted) TYPE set(2) GRANULARITY 1
)
ENGINE = ReplacingMergeTree(version, is_deleted)
ORDER BY (metadata);

-- TTL to clean up deleted rows after 0 seconds (immediate cleanup on merge)
ALTER TABLE TEMPLATE_METADATA MODIFY SETTING allow_experimental_replacing_merge_with_cleanup = 1;
ALTER TABLE TEMPLATE_METADATA MODIFY SETTING deduplicate_merge_projection_mode = 'rebuild';

-- TEMPLATE FOR AUTHORITY TABLES
CREATE TABLE IF NOT EXISTS TEMPLATE_METADATA_AUTHORITY AS TEMPLATE_METADATA;
ALTER TABLE TEMPLATE_METADATA_AUTHORITY
    ADD COLUMN IF NOT EXISTS authority String,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(authority = '', 1, 0),
    ADD PROJECTION IF NOT EXISTS prj_authority (SELECT * ORDER BY (authority, metadata));

/* MINT */
-- 1:1 relationship with metadata account
-- should not have any duplicates, no need for GROUP BY with custom view
CREATE TABLE IF NOT EXISTS metadata_mint_state AS TEMPLATE_METADATA;
ALTER TABLE metadata_mint_state
    ADD COLUMN IF NOT EXISTS mint LowCardinality(String) AFTER metadata,
    ADD PROJECTION IF NOT EXISTS prj_mint (SELECT * ORDER BY (mint));

/* MINT AUTHORITY */
CREATE TABLE IF NOT EXISTS metadata_mint_authority_state AS TEMPLATE_METADATA_AUTHORITY;

/* UPDATE AUTHORITY */
CREATE TABLE IF NOT EXISTS metadata_update_authority_state AS TEMPLATE_METADATA_AUTHORITY;

/* NAME */
CREATE TABLE IF NOT EXISTS metadata_name_state AS TEMPLATE_METADATA;
ALTER TABLE metadata_name_state
    ADD COLUMN IF NOT EXISTS name LowCardinality(String) AFTER metadata,
    ADD PROJECTION IF NOT EXISTS prj_name (SELECT * ORDER BY (name, metadata));

/* SYMBOL */
CREATE TABLE IF NOT EXISTS metadata_symbol_state AS TEMPLATE_METADATA;
ALTER TABLE metadata_symbol_state
    ADD COLUMN IF NOT EXISTS symbol LowCardinality(String) AFTER metadata,
    ADD PROJECTION IF NOT EXISTS prj_symbol (SELECT * ORDER BY (symbol, metadata));

/* URI */
CREATE TABLE IF NOT EXISTS metadata_uri_state AS TEMPLATE_METADATA;
ALTER TABLE metadata_uri_state
    ADD COLUMN IF NOT EXISTS uri LowCardinality(String) AFTER metadata,
    ADD PROJECTION IF NOT EXISTS prj_uri (SELECT * ORDER BY (uri, metadata));

/* ===========================
   MATERIALIZED VIEWS (ROUTING)
   =========================== */

/* INITIALIZE fan-out */
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_initialize_token_metadata_update_authority
TO metadata_update_authority_state AS
SELECT metadata, program_id, update_authority as authority, version, block_num, timestamp
FROM initialize_token_metadata;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_initialize_token_metadata_mint_authority
TO metadata_mint_authority_state AS
SELECT metadata, program_id, mint_authority as authority, version, block_num, timestamp
FROM initialize_token_metadata;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_initialize_token_metadata_mint
TO metadata_mint_state AS
SELECT metadata, program_id, mint, version, block_num, timestamp
FROM initialize_token_metadata;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_initialize_token_metadata_name
TO metadata_name_state AS
SELECT metadata, program_id, name, version, block_num, timestamp
FROM initialize_token_metadata
WHERE name != '';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_initialize_token_metadata_symbol
TO metadata_symbol_state AS
SELECT metadata, program_id, symbol, version, block_num, timestamp
FROM initialize_token_metadata
WHERE symbol != '';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_initialize_token_metadata_uri
TO metadata_uri_state AS
SELECT metadata, program_id, uri, version, block_num, timestamp
FROM initialize_token_metadata
WHERE uri != '';

/* UPDATE AUTHORITY */
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_update_token_metadata_authority
TO metadata_update_authority_state AS
SELECT metadata, program_id, update_authority as authority, version, block_num, timestamp
FROM update_token_metadata_authority;

/* FIELD UPDATES */
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_update_token_metadata_field_name
TO metadata_name_state AS
SELECT metadata, program_id, value AS name, version, block_num, timestamp
FROM update_token_metadata_field
WHERE field = 'name';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_update_token_metadata_field_symbol
TO metadata_symbol_state AS
SELECT metadata, program_id, value AS symbol, version, block_num, timestamp
FROM update_token_metadata_field
WHERE field = 'symbol';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_update_token_metadata_field_uri
TO metadata_uri_state AS
SELECT metadata, program_id, value AS uri, version, block_num, timestamp
FROM update_token_metadata_field
WHERE field = 'uri';

/* FIELD REMOVALS -> emit '' */
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_remove_token_metadata_field_name
TO metadata_name_state AS
SELECT metadata, program_id, '' AS name, version, block_num, timestamp
FROM remove_token_metadata_field
WHERE `key` = 'name';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_remove_token_metadata_field_symbol
TO metadata_symbol_state AS
SELECT metadata, program_id, '' AS symbol, version, block_num, timestamp
FROM remove_token_metadata_field
WHERE `key` = 'symbol';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_remove_token_metadata_field_uri
TO metadata_uri_state AS
SELECT metadata, program_id, '' AS uri, version, block_num, timestamp
FROM remove_token_metadata_field
WHERE `key` = 'uri';
