-- Stabble Swap --
CREATE TABLE IF NOT EXISTS stabble_swap AS base_events
COMMENT 'Stabble Swap';
ALTER TABLE stabble_swap
    ADD COLUMN IF NOT EXISTS user               String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS pool               String COMMENT 'Pool account',
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS minimum_amount_out UInt64 COMMENT 'Minimum amount out';
