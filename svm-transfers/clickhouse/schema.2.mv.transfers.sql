CREATE TABLE IF NOT EXISTS transfers AS spl_transfer
COMMENT 'SPL Token 2022 transfers';

-- SPL Token Transfers --
ALTER TABLE transfers
    -- require `decimals` to be present for token transfers
    DROP COLUMN IF EXISTS decimals,
    DROP COLUMN IF EXISTS decimals_raw,
    ADD COLUMN decimals Nullable(UInt8),

    -- authority --
    DROP COLUMN IF EXISTS multisig_authority,
    DROP COLUMN IF EXISTS multisig_authority_raw,
    ADD COLUMN multisig_authority      Array(String),

    -- Indexes --
    ADD INDEX IF NOT EXISTS idx_amount (amount) TYPE minmax GRANULARITY 1,

    -- PROJECTIONS --
    -- count() --
    ADD PROJECTION IF NOT EXISTS prj_source_count ( SELECT source, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY source ),
    ADD PROJECTION IF NOT EXISTS prj_destination_count ( SELECT destination, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY destination ),
    ADD PROJECTION IF NOT EXISTS prj_mint_count ( SELECT mint, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY mint ),
    ADD PROJECTION IF NOT EXISTS prj_authority_count ( SELECT authority, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY authority ),

    -- minute --
    ADD PROJECTION IF NOT EXISTS prj_source_by_minute ( SELECT source, minute GROUP BY source, minute ),
    ADD PROJECTION IF NOT EXISTS prj_destination_by_minute ( SELECT destination, minute GROUP BY destination, minute ),
    ADD PROJECTION IF NOT EXISTS prj_mint_by_minute ( SELECT mint, minute GROUP BY mint, minute ),
    ADD PROJECTION IF NOT EXISTS prj_authority_by_minute ( SELECT authority, minute GROUP BY authority, minute );

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_spl_transfer
TO transfers AS
SELECT
    * EXCEPT (decimals_raw, multisig_authority_raw),

    -- computed fields --
    decimals AS decimals,
    multisig_authority AS multisig_authority

FROM spl_transfer
-- ignore 0 transfers
WHERE amount > 0 AND mint IS NOT NULL;
