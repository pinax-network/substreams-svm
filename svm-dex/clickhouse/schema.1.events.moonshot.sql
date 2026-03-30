-- Moonshot Buy --
CREATE TABLE IF NOT EXISTS moonshot_buy AS BASE_EVENTS
COMMENT 'Moonshot Buy';
ALTER TABLE moonshot_buy
    ADD COLUMN IF NOT EXISTS amount             UInt64 COMMENT 'Token amount',
    ADD COLUMN IF NOT EXISTS collateral_amount  UInt64 COMMENT 'Collateral amount',
    ADD COLUMN IF NOT EXISTS dex_fee            UInt64 COMMENT 'DEX fee',
    ADD COLUMN IF NOT EXISTS helio_fee          UInt64 COMMENT 'Helio fee',
    ADD COLUMN IF NOT EXISTS sender             String COMMENT 'Sender account',
    ADD COLUMN IF NOT EXISTS trade_type         UInt32 COMMENT 'Trade type',
    ADD COLUMN IF NOT EXISTS cost_token         String COMMENT 'Cost token mint',
    ADD COLUMN IF NOT EXISTS curve              String COMMENT 'Curve account';

-- Moonshot Sell --
CREATE TABLE IF NOT EXISTS moonshot_sell AS BASE_EVENTS
COMMENT 'Moonshot Sell';
ALTER TABLE moonshot_sell
    ADD COLUMN IF NOT EXISTS amount             UInt64 COMMENT 'Token amount',
    ADD COLUMN IF NOT EXISTS collateral_amount  UInt64 COMMENT 'Collateral amount',
    ADD COLUMN IF NOT EXISTS dex_fee            UInt64 COMMENT 'DEX fee',
    ADD COLUMN IF NOT EXISTS helio_fee          UInt64 COMMENT 'Helio fee',
    ADD COLUMN IF NOT EXISTS sender             String COMMENT 'Sender account',
    ADD COLUMN IF NOT EXISTS trade_type         UInt32 COMMENT 'Trade type',
    ADD COLUMN IF NOT EXISTS cost_token         String COMMENT 'Cost token mint',
    ADD COLUMN IF NOT EXISTS curve              String COMMENT 'Curve account';
