-- Heaven Sell --
CREATE TABLE IF NOT EXISTS heaven_sell AS base_events
COMMENT 'Heaven Sell';
ALTER TABLE heaven_sell
    ADD COLUMN IF NOT EXISTS user               String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS mint               String COMMENT 'Token mint',
    ADD COLUMN IF NOT EXISTS amount             UInt64 COMMENT 'Amount';
