-- Boop Buy --
CREATE TABLE IF NOT EXISTS boop_buy AS base_events
COMMENT 'Boop Buy';
ALTER TABLE boop_buy
    ADD COLUMN IF NOT EXISTS mint               String COMMENT 'Token mint',
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS amount_out         UInt64 COMMENT 'Amount out',
    ADD COLUMN IF NOT EXISTS swap_fee           UInt64 COMMENT 'Swap fee',
    ADD COLUMN IF NOT EXISTS buyer              String COMMENT 'Buyer account';

-- Boop Sell --
CREATE TABLE IF NOT EXISTS boop_sell AS base_events
COMMENT 'Boop Sell';
ALTER TABLE boop_sell
    ADD COLUMN IF NOT EXISTS mint               String COMMENT 'Token mint',
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS amount_out         UInt64 COMMENT 'Amount out',
    ADD COLUMN IF NOT EXISTS swap_fee           UInt64 COMMENT 'Swap fee',
    ADD COLUMN IF NOT EXISTS seller             String COMMENT 'Seller account';
