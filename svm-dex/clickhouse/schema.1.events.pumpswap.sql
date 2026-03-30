-- PumpSwap Buy --
CREATE TABLE IF NOT EXISTS pumpswap_buy AS BASE_EVENTS
COMMENT 'PumpSwap Buy';
ALTER TABLE pumpswap_buy
    ADD COLUMN IF NOT EXISTS pool                        String COMMENT 'Pool account',
    ADD COLUMN IF NOT EXISTS user                        String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS base_amount_out             UInt64 COMMENT 'Base amount out',
    ADD COLUMN IF NOT EXISTS quote_amount_in             UInt64 COMMENT 'Quote amount in',
    ADD COLUMN IF NOT EXISTS lp_fee                      UInt64 COMMENT 'LP fee',
    ADD COLUMN IF NOT EXISTS protocol_fee                UInt64 COMMENT 'Protocol fee',
    ADD COLUMN IF NOT EXISTS coin_creator_fee            UInt64 COMMENT 'Coin creator fee',
    ADD COLUMN IF NOT EXISTS pool_base_token_reserves    UInt64 COMMENT 'Pool base token reserves',
    ADD COLUMN IF NOT EXISTS pool_quote_token_reserves   UInt64 COMMENT 'Pool quote token reserves';

-- PumpSwap Sell --
CREATE TABLE IF NOT EXISTS pumpswap_sell AS BASE_EVENTS
COMMENT 'PumpSwap Sell';
ALTER TABLE pumpswap_sell
    ADD COLUMN IF NOT EXISTS pool                        String COMMENT 'Pool account',
    ADD COLUMN IF NOT EXISTS user                        String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS base_amount_in              UInt64 COMMENT 'Base amount in',
    ADD COLUMN IF NOT EXISTS quote_amount_out            UInt64 COMMENT 'Quote amount out',
    ADD COLUMN IF NOT EXISTS lp_fee                      UInt64 COMMENT 'LP fee',
    ADD COLUMN IF NOT EXISTS protocol_fee                UInt64 COMMENT 'Protocol fee',
    ADD COLUMN IF NOT EXISTS coin_creator_fee            UInt64 COMMENT 'Coin creator fee',
    ADD COLUMN IF NOT EXISTS pool_base_token_reserves    UInt64 COMMENT 'Pool base token reserves',
    ADD COLUMN IF NOT EXISTS pool_quote_token_reserves   UInt64 COMMENT 'Pool quote token reserves';
