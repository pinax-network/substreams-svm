-- Obric Swap --
CREATE TABLE IF NOT EXISTS obric_swap AS BASE_EVENTS
COMMENT 'Obric Swap';
ALTER TABLE obric_swap
    ADD COLUMN IF NOT EXISTS input_amount       UInt64 COMMENT 'Input amount',
    ADD COLUMN IF NOT EXISTS min_output_amount  UInt64 COMMENT 'Minimum output amount';
