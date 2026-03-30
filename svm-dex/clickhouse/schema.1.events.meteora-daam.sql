-- Meteora DAAM Swap --
CREATE TABLE IF NOT EXISTS meteora_daam_swap AS base_events
COMMENT 'Meteora DAAM Swap';
ALTER TABLE meteora_daam_swap
    ADD COLUMN IF NOT EXISTS payer       String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS pool        String COMMENT 'Pool account',
    ADD COLUMN IF NOT EXISTS input_mint  String COMMENT 'Input token mint',
    ADD COLUMN IF NOT EXISTS output_mint String COMMENT 'Output token mint',
    ADD COLUMN IF NOT EXISTS amount_in   UInt64 COMMENT 'Amount of tokens in',
    ADD COLUMN IF NOT EXISTS amount_out  UInt64 COMMENT 'Amount of tokens out';
