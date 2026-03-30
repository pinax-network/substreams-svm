-- Meteora DLLM Swap --
CREATE TABLE IF NOT EXISTS meteora_dllm_swap AS BASE_EVENTS
COMMENT 'Meteora DLLM Swap';
ALTER TABLE meteora_dllm_swap
    ADD COLUMN IF NOT EXISTS user        String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS lb_pair     String COMMENT 'Liquidity pair',
    ADD COLUMN IF NOT EXISTS input_mint  String COMMENT 'Input token mint',
    ADD COLUMN IF NOT EXISTS output_mint String COMMENT 'Output token mint',
    ADD COLUMN IF NOT EXISTS amount_in   UInt64 COMMENT 'Amount of tokens in',
    ADD COLUMN IF NOT EXISTS amount_out  UInt64 COMMENT 'Amount of tokens out';
