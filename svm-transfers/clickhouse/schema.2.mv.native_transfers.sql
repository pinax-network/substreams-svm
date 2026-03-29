CREATE TABLE IF NOT EXISTS native_transfers AS system_transfer
COMMENT 'Native transfers';

-- Native Token Transfers --
ALTER TABLE native_transfers
    -- Indexes --
    ADD INDEX IF NOT EXISTS idx_lamports (lamports) TYPE minmax GRANULARITY 1,

    -- PROJECTIONS --
    -- count() --
    PROJECTION prj_source_count ( SELECT source, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY source ),
    PROJECTION prj_destination_count ( SELECT destination, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY destination ),

    -- minute --
    PROJECTION prj_source_by_minute ( SELECT source, minute GROUP BY source, minute ),
    PROJECTION prj_destination_by_minute ( SELECT destination, minute GROUP BY destination, minute );

-- System Token Transfers --
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_system_transfer
TO native_transfers AS
SELECT *
FROM system_transfer
-- ignore 0 transfers
WHERE lamports > 0;

-- TransferWithSeed --
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_system_transfer_with_seed
TO native_transfers AS
SELECT
    * EXCEPT (lamports, source_base, source_seed, source_owner),
FROM system_transfer_with_seed
-- ignore 0 transfers
WHERE lamports > 0;

-- WithdrawNonceAccount --
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_system_withdraw_nonce_account
TO native_transfers AS
SELECT
    * EXCEPT (lamports, nonce_account, nonce_authority),
FROM system_withdraw_nonce_account
-- ignore 0 transfers
WHERE lamports > 0;