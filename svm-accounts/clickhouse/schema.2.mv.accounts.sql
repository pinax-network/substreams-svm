CREATE TABLE IF NOT EXISTS TEMPLATE_ACCOUNTS_STATE (
    account      String,
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
ORDER BY (account);

-- TTL to clean up deleted rows after 0 seconds (immediate cleanup on merge)
ALTER TABLE TEMPLATE_ACCOUNTS_STATE MODIFY SETTING allow_experimental_replacing_merge_with_cleanup = 1;
ALTER TABLE TEMPLATE_ACCOUNTS_STATE MODIFY SETTING deduplicate_merge_projection_mode = 'rebuild';

-- ACCOUNT (CREATED AT)
CREATE TABLE IF NOT EXISTS account_state AS TEMPLATE_ACCOUNTS_STATE;

-- OWNER
CREATE TABLE IF NOT EXISTS owner_state AS TEMPLATE_ACCOUNTS_STATE;
ALTER TABLE owner_state
    ADD COLUMN IF NOT EXISTS owner String,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(owner = '', 1, 0),
    ADD PROJECTION IF NOT EXISTS prj_owner (SELECT * ORDER BY owner);

-- MINT
CREATE TABLE IF NOT EXISTS account_mint_state AS TEMPLATE_ACCOUNTS_STATE;
ALTER TABLE account_mint_state
    ADD COLUMN IF NOT EXISTS mint LowCardinality(String),
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(mint = '', 1, 0),
    ADD PROJECTION IF NOT EXISTS prj_mint (SELECT * ORDER BY (mint, account));

-- CLOSE ACCOUNT (0/1)
CREATE TABLE IF NOT EXISTS close_account_state AS TEMPLATE_ACCOUNTS_STATE;
ALTER TABLE close_account_state
    ADD COLUMN IF NOT EXISTS closed UInt8,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(closed = 0, 1, 0);

-- FROZEN (0/1)
CREATE TABLE IF NOT EXISTS freeze_account_state AS TEMPLATE_ACCOUNTS_STATE;
ALTER TABLE freeze_account_state
    ADD COLUMN IF NOT EXISTS frozen UInt8,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(frozen = 0, 1, 0);

-- IMMUTABLE OWNER (0/1)
CREATE TABLE IF NOT EXISTS immutable_owner_state AS TEMPLATE_ACCOUNTS_STATE;
ALTER TABLE immutable_owner_state
    ADD COLUMN IF NOT EXISTS immutable UInt8,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(immutable = 0, 1, 0);

-- CLOSE ACCOUNT AUTHORITY
CREATE TABLE IF NOT EXISTS close_account_authority_state AS TEMPLATE_ACCOUNTS_STATE;
ALTER TABLE close_account_authority_state
    ADD COLUMN IF NOT EXISTS authority String,
    MODIFY COLUMN is_deleted UInt8 MATERIALIZED if(authority = '', 1, 0),
    ADD PROJECTION IF NOT EXISTS prj_authority (SELECT * ORDER BY (authority, account));

-- INITIALIZE
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_owner_state_initialize_owner
TO owner_state AS
SELECT
  program_id,
  account,
  owner,
  version,
  block_num,
  timestamp
FROM initialize_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_account_state_initialize_created
TO account_state AS
SELECT
  program_id,
  account,
  version,
  block_num,
  timestamp
FROM initialize_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_mint_state_initialize_mint
TO account_mint_state AS
SELECT
  program_id,
  account,
  mint,
  version,
  block_num,
  timestamp
FROM initialize_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_close_account_state_initialize_closed0
TO close_account_state AS
SELECT
  program_id,
  account,
  0 as closed,
  version,
  block_num,
  timestamp
FROM initialize_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_freeze_account_state_initialize_frozen0
TO freeze_account_state AS
SELECT
  program_id,
  account,
  0 as frozen,
  version,
  block_num,
  timestamp
FROM initialize_account;

-- CLOSE -> closed = 1 (do not touch others)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_close_account_state_close_account
TO close_account_state AS
SELECT
  program_id,
  account,
  1 as closed,
  version,
  block_num,
  timestamp
FROM close_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_close_account_state_close_account_owner
TO owner_state AS
SELECT
  program_id,
  account,
  '' as owner,
  version,
  block_num,
  timestamp
FROM close_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_close_account_state_close_account_mint
TO account_mint_state AS
SELECT
  program_id,
  account,
  '' as mint,
  version,
  block_num,
  timestamp
FROM close_account;

-- SET AUTHORITY (owner)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_owner_state_set_authority_owner
TO owner_state AS
SELECT
  program_id,
  account,
  new_authority as owner,
  version,
  block_num,
  timestamp
FROM set_authority
WHERE authority_type = 'AUTHORITY_TYPE_ACCOUNT_OWNER';

-- FREEZE / THAW
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_freeze_account_state_freeze_account
TO freeze_account_state AS
SELECT
  program_id,
  account,
  1 as frozen,
  version,
  block_num,
  timestamp
FROM freeze_account;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_freeze_account_state_thaw_account
TO freeze_account_state AS
SELECT
  program_id,
  account,
  0 as frozen,
  version,
  block_num,
  timestamp
FROM thaw_account;

-- IMMUTABLE OWNER
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_immutable_owner_state_set_authority_immutable_owner
TO immutable_owner_state AS
SELECT
  program_id,
  account,
  1 as immutable,
  version,
  block_num,
  timestamp
FROM initialize_immutable_owner;
