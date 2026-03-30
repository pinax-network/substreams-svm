-- Meteora AMM Swap --
CREATE TABLE IF NOT EXISTS meteora_amm_swap AS BASE_EVENTS
COMMENT 'Meteora AMM Swap';
ALTER TABLE meteora_amm_swap
    ADD COLUMN IF NOT EXISTS user        String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS pool        String COMMENT 'Pool account',
    ADD COLUMN IF NOT EXISTS input_mint  String COMMENT 'Input token account',
    ADD COLUMN IF NOT EXISTS output_mint String COMMENT 'Output token account',
    ADD COLUMN IF NOT EXISTS amount_in   UInt64 COMMENT 'Amount of tokens in',
    ADD COLUMN IF NOT EXISTS amount_out  UInt64 COMMENT 'Amount of tokens out';
