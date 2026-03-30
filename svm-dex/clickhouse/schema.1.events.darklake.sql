-- Darklake Swap --
CREATE TABLE IF NOT EXISTS darklake_swap AS BASE_EVENTS
COMMENT 'Darklake Swap';
ALTER TABLE darklake_swap
    ADD COLUMN IF NOT EXISTS trader        String COMMENT 'Trader account',
    ADD COLUMN IF NOT EXISTS amount_in     UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS amount_out    UInt64 COMMENT 'Amount out',
    ADD COLUMN IF NOT EXISTS token_mint_x  String COMMENT 'Token mint X',
    ADD COLUMN IF NOT EXISTS token_mint_y  String COMMENT 'Token mint Y',
    ADD COLUMN IF NOT EXISTS direction     UInt32 COMMENT 'Swap direction',
    ADD COLUMN IF NOT EXISTS trade_fee     UInt64 COMMENT 'Trade fee',
    ADD COLUMN IF NOT EXISTS protocol_fee  UInt64 COMMENT 'Protocol fee';
