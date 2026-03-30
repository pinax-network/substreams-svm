-- Jupiter V4 & V6 Swaps --
CREATE TABLE IF NOT EXISTS jupiter_swap AS base_events
COMMENT 'Jupiter V4 & V6 Swaps';
ALTER TABLE jupiter_swap
    -- log --
    ADD COLUMN IF NOT EXISTS amm                         String COMMENT 'AMM pool account (Raydium V4)',
    ADD COLUMN IF NOT EXISTS input_mint                  String COMMENT 'Input token mint address',
    ADD COLUMN IF NOT EXISTS input_amount                UInt64 COMMENT 'Amount of input tokens swapped',
    ADD COLUMN IF NOT EXISTS output_mint                 String COMMENT 'Output token mint address',
    ADD COLUMN IF NOT EXISTS output_amount               UInt64 COMMENT 'Amount of output tokens received';
