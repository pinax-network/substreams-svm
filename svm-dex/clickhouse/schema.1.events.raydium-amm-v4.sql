-- Raydium AMM V4 Swap --
CREATE TABLE IF NOT EXISTS raydium_amm_v4_swap_base_in AS BASE_EVENTS
COMMENT 'Raydium AMM V4 Swap';
ALTER TABLE raydium_amm_v4_swap_base_in
    -- accounts --
    ADD COLUMN IF NOT EXISTS token_program               String COMMENT 'Token program (usually SPL Token)',
    ADD COLUMN IF NOT EXISTS amm                         String COMMENT 'AMM pool account (Raydium V4 liquidity-state)',
    ADD COLUMN IF NOT EXISTS amm_authority               String COMMENT 'AMM authority PDA',
    ADD COLUMN IF NOT EXISTS amm_open_orders             String COMMENT 'AMM open-orders',
    ADD COLUMN IF NOT EXISTS amm_target_orders           String COMMENT 'AMM target-orders',
    ADD COLUMN IF NOT EXISTS amm_coin_vault              String COMMENT 'AMM coin vault (base-token vault)',
    ADD COLUMN IF NOT EXISTS amm_pc_vault                String COMMENT 'AMM pc vault (quote-token vault)',
    ADD COLUMN IF NOT EXISTS market_program              String COMMENT 'OpenBook (or Serum) DEX program',
    ADD COLUMN IF NOT EXISTS market                      String COMMENT 'OpenBook (or Serum) DEX market account',
    ADD COLUMN IF NOT EXISTS market_bids                 String COMMENT 'Market account',
    ADD COLUMN IF NOT EXISTS market_asks                 String COMMENT 'Market bids slab',
    ADD COLUMN IF NOT EXISTS market_event_queue          String COMMENT 'Market asks slab',
    ADD COLUMN IF NOT EXISTS market_coin_vault           String COMMENT 'Market event queue',
    ADD COLUMN IF NOT EXISTS market_pc_vault             String COMMENT 'Market pc vault (quote)',
    ADD COLUMN IF NOT EXISTS market_vault_signer         String COMMENT 'Market vault-signer PDA',
    ADD COLUMN IF NOT EXISTS user_token_source           String COMMENT 'User source ATA (base token)',
    ADD COLUMN IF NOT EXISTS user_token_destination      String COMMENT 'User destination ATA (quote token)',
    ADD COLUMN IF NOT EXISTS user_source_owner           String COMMENT 'User wallet (authority & fee-payer)',

    -- data --
    ADD COLUMN IF NOT EXISTS amount_in                   UInt64,
    ADD COLUMN IF NOT EXISTS minimum_amount_out          UInt64,

    -- log --
    ADD COLUMN IF NOT EXISTS amount_out                  UInt64,
    ADD COLUMN IF NOT EXISTS direction                   Enum8('PC2Coin' = 1, 'Coin2PC' = 2),
    ADD COLUMN IF NOT EXISTS user_source                 UInt64,
    ADD COLUMN IF NOT EXISTS pool_coin                   UInt64,
    ADD COLUMN IF NOT EXISTS pool_pc                     UInt64;

--- SwapBaseOut --
CREATE TABLE IF NOT EXISTS raydium_amm_v4_swap_base_out AS raydium_amm_v4_swap_base_in;
ALTER TABLE raydium_amm_v4_swap_base_out
    RENAME COLUMN IF EXISTS minimum_amount_out TO max_amount_in;
