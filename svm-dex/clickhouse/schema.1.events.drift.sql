-- Drift Swap --
CREATE TABLE IF NOT EXISTS drift_swap AS base_events
COMMENT 'Drift Swap';
ALTER TABLE drift_swap
    ADD COLUMN IF NOT EXISTS user               String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS amount_out         UInt64 COMMENT 'Amount out';
