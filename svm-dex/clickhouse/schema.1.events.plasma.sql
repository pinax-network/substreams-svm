-- Plasma Buy --
CREATE TABLE IF NOT EXISTS plasma_buy AS BASE_EVENTS
COMMENT 'Plasma Buy';
ALTER TABLE plasma_buy
    ADD COLUMN IF NOT EXISTS side               UInt32 COMMENT 'Side (0=Buy, 1=Sell)',
    ADD COLUMN IF NOT EXISTS swap_type          UInt32 COMMENT 'Swap type (0=ExactIn, 1=ExactOut)',
    ADD COLUMN IF NOT EXISTS amount             UInt64 COMMENT 'Amount',
    ADD COLUMN IF NOT EXISTS limit_amount       UInt64 COMMENT 'Limit amount';

-- Plasma Sell --
CREATE TABLE IF NOT EXISTS plasma_sell AS BASE_EVENTS
COMMENT 'Plasma Sell';
ALTER TABLE plasma_sell
    ADD COLUMN IF NOT EXISTS side               UInt32 COMMENT 'Side (0=Buy, 1=Sell)',
    ADD COLUMN IF NOT EXISTS swap_type          UInt32 COMMENT 'Swap type (0=ExactIn, 1=ExactOut)',
    ADD COLUMN IF NOT EXISTS amount             UInt64 COMMENT 'Amount',
    ADD COLUMN IF NOT EXISTS limit_amount       UInt64 COMMENT 'Limit amount';
