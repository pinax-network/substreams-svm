-- SPL Token Transfers --
CREATE TABLE IF NOT EXISTS spl_transfer AS BASE_EVENTS
COMMENT 'SPL Token Transfer/Burn/Mint events';
ALTER TABLE spl_transfer
    -- authority --
    ADD COLUMN IF NOT EXISTS authority               String,
    ADD COLUMN IF NOT EXISTS multisig_authority_raw  String,

    -- events --
    ADD COLUMN IF NOT EXISTS source                  String,
    ADD COLUMN IF NOT EXISTS destination             String,
    ADD COLUMN IF NOT EXISTS amount                  UInt64,
    ADD COLUMN IF NOT EXISTS mint                    LowCardinality(String),

    -- Optional
    ADD COLUMN IF NOT EXISTS decimals_raw            String;
