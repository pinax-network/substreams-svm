-- Serum Swap --
CREATE TABLE IF NOT EXISTS serum_swap AS BASE_EVENTS
COMMENT 'Serum Swap';
ALTER TABLE serum_swap
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS minimum_amount_out UInt64 COMMENT 'Minimum amount out';
