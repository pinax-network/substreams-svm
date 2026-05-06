-- SVM Swaps --
CREATE TABLE IF NOT EXISTS swaps AS BASE_EVENTS
COMMENT 'Solana Swaps';

ALTER TABLE swaps
    -- log --
    ADD COLUMN IF NOT EXISTS protocol                    Enum8(
        'unspecified' = 0,
        'boop' = 1,
        'darklake' = 2,
        'dumpfun' = 3,
        'jupiter_v4' = 4,
        'jupiter_v6' = 5,
        'meteora_daam' = 6,
        'meteora_dlmm' = 7,
        'orca_whirlpool' = 8,
        'pumpfun' = 9,
        'pumpfun_amm' = 10,
        'raydium_amm_v4' = 11,
        'raydium_clmm' = 12,
        'raydium_cpmm' = 13,
        'raydium_launchpad' = 14,
        'meteora_amm' = 15,
        'byreal' = 16,
        'moonshot' = 17,
        'pancakeswap' = 18,
        'spl_token_swap' = 19,
        'okx_dex' = 20
    ) COMMENT 'Protocol',
    ADD COLUMN IF NOT EXISTS amm                         String COMMENT 'AMM protocol (Raydium Liquidity Pool V4)',
    ADD COLUMN IF NOT EXISTS amm_pool                    String COMMENT 'AMM market (Raydium "WSOL-USDT" Market)',
    ADD COLUMN IF NOT EXISTS user                        String COMMENT 'User wallet address',
    ADD COLUMN IF NOT EXISTS input_mint                  String COMMENT 'Input token mint address',
    ADD COLUMN IF NOT EXISTS input_amount                UInt64 COMMENT 'Amount of input tokens swapped',
    ADD COLUMN IF NOT EXISTS output_mint                 String COMMENT 'Output token mint address',
    ADD COLUMN IF NOT EXISTS output_amount               UInt64 COMMENT 'Amount of output tokens received',

    -- indexes --
    ADD INDEX IF NOT EXISTS         idx_input_amount                (input_amount) TYPE minmax GRANULARITY 1,
    ADD INDEX IF NOT EXISTS         idx_output_amount               (output_amount) TYPE minmax GRANULARITY 1,

    -- count() --
    ADD PROJECTION IF NOT EXISTS    prj_protocol_count              ( SELECT protocol, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY protocol ),
    ADD PROJECTION IF NOT EXISTS    prj_amm_count                   ( SELECT amm, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY amm ),
    ADD PROJECTION IF NOT EXISTS    prj_amm_pool_count              ( SELECT amm_pool, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY amm_pool ),
    ADD PROJECTION IF NOT EXISTS    prj_user_count                  ( SELECT user, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY user ),
    ADD PROJECTION IF NOT EXISTS    prj_input_mint_count            ( SELECT input_mint, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY input_mint ),
    ADD PROJECTION IF NOT EXISTS    prj_output_mint_count           ( SELECT output_mint, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY output_mint ),

    -- minute --
    ADD PROJECTION IF NOT EXISTS    prj_protocol_by_minute          ( SELECT protocol, minute GROUP BY protocol, minute ),
    ADD PROJECTION IF NOT EXISTS    prj_amm_by_minute               ( SELECT amm, minute GROUP BY amm, minute ),
    ADD PROJECTION IF NOT EXISTS    prj_amm_pool_by_minute          ( SELECT amm_pool, minute GROUP BY amm_pool, minute ),
    ADD PROJECTION IF NOT EXISTS    prj_user_by_minute              ( SELECT user, minute GROUP BY user, minute ),
    ADD PROJECTION IF NOT EXISTS    prj_input_mint_by_minute        ( SELECT input_mint, minute GROUP BY input_mint, minute ),
    ADD PROJECTION IF NOT EXISTS    prj_output_mint_by_minute       ( SELECT output_mint, minute GROUP BY output_mint, minute );
