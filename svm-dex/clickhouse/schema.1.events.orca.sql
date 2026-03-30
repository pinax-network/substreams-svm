-- Orca Swap --
CREATE TABLE IF NOT EXISTS orca_swap AS base_events
COMMENT 'Orca Whirlpool Swap';
ALTER TABLE orca_swap
    ADD COLUMN IF NOT EXISTS user         String COMMENT 'User (token authority)',
    ADD COLUMN IF NOT EXISTS whirlpool    String COMMENT 'Whirlpool account',
    ADD COLUMN IF NOT EXISTS input_mint   String COMMENT 'Input token mint',
    ADD COLUMN IF NOT EXISTS output_mint  String COMMENT 'Output token mint',
    ADD COLUMN IF NOT EXISTS amount_in    UInt64 COMMENT 'Amount of tokens in',
    ADD COLUMN IF NOT EXISTS amount_out   UInt64 COMMENT 'Amount of tokens out';
