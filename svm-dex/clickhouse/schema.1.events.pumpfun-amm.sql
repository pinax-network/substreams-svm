-- Pump.fun AMM Swap Buy --
CREATE TABLE IF NOT EXISTS pumpfun_amm_buy AS BASE_EVENTS
COMMENT 'Pump.fun AMM Swap';
ALTER TABLE pumpfun_amm_buy
    -- data --
    ADD COLUMN IF NOT EXISTS base_amount_out         UInt64 COMMENT 'Amount of base tokens swapped out',
    ADD COLUMN IF NOT EXISTS max_quote_amount_in     UInt64 COMMENT 'Maximum amount of quote tokens to swap in',

    -- accounts --
    ADD COLUMN IF NOT EXISTS pool                                   String COMMENT 'AMM pool account',
    ADD COLUMN IF NOT EXISTS user                                   String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS global_config                          String COMMENT 'Global config account',
    ADD COLUMN IF NOT EXISTS base_mint                              String COMMENT 'Base token mint address',
    ADD COLUMN IF NOT EXISTS quote_mint                             String COMMENT 'Quote token mint address',
    ADD COLUMN IF NOT EXISTS user_base_token_account                String COMMENT 'User base token account',
    ADD COLUMN IF NOT EXISTS user_quote_token_account               String COMMENT 'User quote token account',
    ADD COLUMN IF NOT EXISTS pool_base_token_account                String COMMENT 'Pool base token account',
    ADD COLUMN IF NOT EXISTS pool_quote_token_account               String COMMENT 'Pool quote token account',
    ADD COLUMN IF NOT EXISTS protocol_fee_recipient                 String COMMENT 'Protocol fee recipient account',
    ADD COLUMN IF NOT EXISTS protocol_fee_recipient_token_account   String COMMENT 'Protocol fee recipient token account',
    ADD COLUMN IF NOT EXISTS coin_creator_vault_ata                 String DEFAULT '' COMMENT 'Coin creator vault ATA',
    ADD COLUMN IF NOT EXISTS coin_creator_vault_authority           String DEFAULT '' COMMENT 'Coin creator vault authority',

    -- event --
    ADD COLUMN IF NOT EXISTS quote_amount_in                UInt64 COMMENT 'Amount of quote tokens swapped in',
    ADD COLUMN IF NOT EXISTS quote_amount_in_with_lp_fee    UInt64 COMMENT 'Amount of quote tokens swapped in with LP fee',
    ADD COLUMN IF NOT EXISTS user_quote_amount_in           UInt64 COMMENT 'Amount of quote tokens swapped in by user';

-- Pump.fun AMM Swap Sell --
CREATE TABLE IF NOT EXISTS pumpfun_amm_sell AS pumpfun_amm_buy;
ALTER TABLE pumpfun_amm_sell
    RENAME COLUMN IF EXISTS base_amount_out TO base_amount_in,
    RENAME COLUMN IF EXISTS max_quote_amount_in TO min_quote_amount_out,
    RENAME COLUMN IF EXISTS quote_amount_in TO quote_amount_out,
    RENAME COLUMN IF EXISTS quote_amount_in_with_lp_fee TO quote_amount_out_without_lp_fee,
    RENAME COLUMN IF EXISTS user_quote_amount_in TO user_quote_amount_out;
