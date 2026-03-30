-- Raydium CLMM Swap --
CREATE TABLE IF NOT EXISTS raydium_clmm_swap AS BASE_EVENTS
COMMENT 'Raydium CLMM Swap';
ALTER TABLE raydium_clmm_swap
    ADD COLUMN IF NOT EXISTS payer        String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS pool_state   String COMMENT 'Pool state account',
    ADD COLUMN IF NOT EXISTS input_mint   String COMMENT 'Input token mint or vault',
    ADD COLUMN IF NOT EXISTS output_mint  String COMMENT 'Output token mint or vault',
    ADD COLUMN IF NOT EXISTS amount_in    UInt64 COMMENT 'Amount of tokens in',
    ADD COLUMN IF NOT EXISTS amount_out   UInt64 COMMENT 'Amount of tokens out';
