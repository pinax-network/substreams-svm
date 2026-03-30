-- DFlow Swap --
CREATE TABLE IF NOT EXISTS dflow_swap AS BASE_EVENTS
COMMENT 'DFlow Swap';
ALTER TABLE dflow_swap
    ADD COLUMN IF NOT EXISTS amount_in          UInt64 COMMENT 'Amount in',
    ADD COLUMN IF NOT EXISTS minimum_amount_out UInt64 COMMENT 'Minimum amount out';
