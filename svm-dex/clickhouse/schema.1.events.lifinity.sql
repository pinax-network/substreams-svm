-- Lifinity Swap --
CREATE TABLE IF NOT EXISTS lifinity_swap AS BASE_EVENTS
COMMENT 'Lifinity Swap';
ALTER TABLE lifinity_swap
    ADD COLUMN IF NOT EXISTS user               String COMMENT 'User transfer authority',
    ADD COLUMN IF NOT EXISTS amm                String COMMENT 'AMM account',
    ADD COLUMN IF NOT EXISTS swap_source        String COMMENT 'Swap source account',
    ADD COLUMN IF NOT EXISTS swap_destination   String COMMENT 'Swap destination account',
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS minimum_amount_out UInt64 COMMENT 'Minimum amount out';
