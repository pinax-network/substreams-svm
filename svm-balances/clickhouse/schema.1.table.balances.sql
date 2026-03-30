-- SPL Token Post Balance --
CREATE TABLE IF NOT EXISTS post_token_balances AS BASE_TRANSACTIONS
COMMENT 'SPL Token Post Balance events (only last transaction in block which effects the balance)';
ALTER TABLE post_token_balances
    ADD COLUMN IF NOT EXISTS program_id         LowCardinality(String) COMMENT 'Program ID of the SPL Token program.',
    ADD COLUMN IF NOT EXISTS account            String COMMENT 'Account address.',
    ADD COLUMN IF NOT EXISTS mint               String COMMENT 'Mint address',
    ADD COLUMN IF NOT EXISTS amount             UInt64 COMMENT 'Balance amount in lamports.',
    ADD COLUMN IF NOT EXISTS decimals           UInt8;

-- System Post Balance --
CREATE TABLE IF NOT EXISTS system_post_balances AS BASE_TRANSACTIONS
COMMENT 'System post balances (only last transaction in block which effects the balance)';
ALTER TABLE system_post_balances
    ADD COLUMN IF NOT EXISTS account                  String COMMENT 'Account address.',
    ADD COLUMN IF NOT EXISTS amount                   UInt64 COMMENT 'Balance amount in lamports.';
