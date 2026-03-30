-- Phoenix Swap --
CREATE TABLE IF NOT EXISTS phoenix_swap AS BASE_EVENTS
COMMENT 'Phoenix Swap';
ALTER TABLE phoenix_swap
    ADD COLUMN IF NOT EXISTS trader         String COMMENT 'Trader account',
    ADD COLUMN IF NOT EXISTS market         String COMMENT 'Market account',
    ADD COLUMN IF NOT EXISTS base_account   String COMMENT 'Base token account',
    ADD COLUMN IF NOT EXISTS quote_account  String COMMENT 'Quote token account';
