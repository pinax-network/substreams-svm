-- PancakeSwap Swap --
CREATE TABLE IF NOT EXISTS pancakeswap_swap AS base_events
COMMENT 'PancakeSwap Swap';
ALTER TABLE pancakeswap_swap
    ADD COLUMN IF NOT EXISTS pool_state       String COMMENT 'Pool state account',
    ADD COLUMN IF NOT EXISTS sender           String COMMENT 'Sender account',
    ADD COLUMN IF NOT EXISTS amount_0         UInt64 COMMENT 'Amount 0',
    ADD COLUMN IF NOT EXISTS amount_1         UInt64 COMMENT 'Amount 1',
    ADD COLUMN IF NOT EXISTS zero_for_one     Bool COMMENT 'Zero for one direction',
    ADD COLUMN IF NOT EXISTS tick             Int32 COMMENT 'Tick',
    ADD COLUMN IF NOT EXISTS sqrt_price_x64   String COMMENT 'Sqrt price X64',
    ADD COLUMN IF NOT EXISTS liquidity        String COMMENT 'Liquidity';
