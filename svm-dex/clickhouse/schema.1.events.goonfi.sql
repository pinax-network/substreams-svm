-- GoonFi Buy --
CREATE TABLE IF NOT EXISTS goonfi_buy AS BASE_EVENTS
COMMENT 'GoonFi Buy';
ALTER TABLE goonfi_buy
    ADD COLUMN IF NOT EXISTS is_bid             Bool COMMENT 'Is bid';

-- GoonFi Sell --
CREATE TABLE IF NOT EXISTS goonfi_sell AS BASE_EVENTS
COMMENT 'GoonFi Sell';
ALTER TABLE goonfi_sell
    ADD COLUMN IF NOT EXISTS is_bid             Bool COMMENT 'Is bid';
