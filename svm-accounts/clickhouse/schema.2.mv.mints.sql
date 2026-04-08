-- TEMPLATE FOR MINT TABLES
CREATE TABLE IF NOT EXISTS TEMPLATE_MINTS_STATE (
    mint         String,
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
ORDER BY (mint);

-- TTL to clean up deleted rows after 0 seconds (immediate cleanup on merge)
ALTER TABLE TEMPLATE_MINTS_STATE MODIFY SETTING allow_experimental_replacing_merge_with_cleanup = 1;
ALTER TABLE TEMPLATE_MINTS_STATE MODIFY SETTING deduplicate_merge_projection_mode = 'rebuild';

-- TEMPLATE FOR AUTHORITY TABLES
CREATE TABLE IF NOT EXISTS TEMPLATE_MINTS_AUTHORITY_STATE AS TEMPLATE_MINTS_STATE;
ALTER TABLE TEMPLATE_MINTS_AUTHORITY_STATE
    ADD COLUMN IF NOT EXISTS authority String,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(authority = '', 1, 0),
    ADD PROJECTION IF NOT EXISTS prj_authority (SELECT * ORDER BY (authority, mint));

-- MINT AUTHORITY
CREATE TABLE IF NOT EXISTS mint_authority_state AS TEMPLATE_MINTS_AUTHORITY_STATE;

-- FREEZE AUTHORITY
CREATE TABLE IF NOT EXISTS freeze_authority_state AS TEMPLATE_MINTS_AUTHORITY_STATE;

-- CLOSE MINT AUTHORITY
CREATE TABLE IF NOT EXISTS close_mint_authority_state AS TEMPLATE_MINTS_AUTHORITY_STATE;

-- DECIMALS
CREATE TABLE IF NOT EXISTS decimals_state AS TEMPLATE_MINTS_STATE;
ALTER TABLE decimals_state
    ADD COLUMN IF NOT EXISTS decimals UInt8,
    ADD INDEX IF NOT EXISTS idx_decimals (decimals) TYPE minmax GRANULARITY 1;

-- CLOSED MINT (0/1)
CREATE TABLE IF NOT EXISTS close_mint_state AS TEMPLATE_MINTS_STATE;
ALTER TABLE close_mint_state
    ADD COLUMN IF NOT EXISTS closed UInt8,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(closed = 0, 1, 0);

-- INITIALIZE
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_decimals_state_initialize_mint
TO decimals_state AS
SELECT
  program_id,
  mint,
  decimals,
  version,
  block_num,
  timestamp
FROM initialize_mint;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_mint_authority_state_initialize_mint
TO mint_authority_state AS
SELECT
  program_id,
  mint,
  mint_authority as authority,
  version,
  block_num,
  timestamp
FROM initialize_mint;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_freeze_authority_state_initialize_mint
TO freeze_authority_state AS
SELECT
  program_id,
  mint,
  freeze_authority_raw AS authority,
  version,
  block_num,
  timestamp
FROM initialize_mint;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_close_mint_state_initialize_mint
TO close_mint_state AS
SELECT
  program_id,
  mint,
  0 as closed,
  version,
  block_num,
  timestamp
FROM initialize_mint;
