CREATE TABLE IF NOT EXISTS transfers AS spl_transfer
COMMENT 'SPL Token 2022 transfers';

-- SPL Token Transfers --
ALTER TABLE transfers
    -- require `decimals` to be present for token transfers
    DROP COLUMN IF EXISTS decimals,
    DROP COLUMN IF EXISTS decimals_raw,
    ADD COLUMN decimals Nullable(UInt8),

    -- authority --
    DROP COLUMN IF EXISTS multisig_authority_raw,
    DROP COLUMN IF EXISTS multisig_authority,
    ADD COLUMN multisig_authority      Array(String),

    -- Indexes --
    ADD INDEX IF NOT EXISTS idx_amount (amount) TYPE minmax GRANULARITY 1,

    -- PROJECTIONS --
    -- count() --
    PROJECTION prj_source_count ( SELECT source, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY source ),
    PROJECTION prj_destination_count ( SELECT destination, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY destination ),
    PROJECTION prj_mint_count ( SELECT mint, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY mint ),
    PROJECTION prj_authority_count ( SELECT authority, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY authority ),

    -- minute --
    PROJECTION prj_source_by_minute ( SELECT source, minute GROUP BY source, minute ),
    PROJECTION prj_destination_by_minute ( SELECT destination, minute GROUP BY destination, minute ),
    PROJECTION prj_mint_by_minute ( SELECT mint, minute GROUP BY mint, minute ),
    PROJECTION prj_authority_by_minute ( SELECT authority, minute GROUP BY authority, minute );

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_spl_transfer
TO transfers AS
SELECT
    * EXCEPT (decimals_raw),

    -- computed fields --
    decimals AS decimals,
    multisig_authority AS multisig_authority

FROM spl_transfer
-- ignore 0 transfers
WHERE amount > 0 AND mint IS NOT NULL;
