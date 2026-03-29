-- System Token Transfers --
CREATE TABLE IF NOT EXISTS system_transfer AS BASE_EVENTS
COMMENT 'System token transfer';
ALTER TABLE system_transfer
    ADD COLUMN IF NOT EXISTS source                  String,
    ADD COLUMN IF NOT EXISTS destination             String,
    ADD COLUMN IF NOT EXISTS lamports                UInt64;

-- TransferWithSeed --
CREATE TABLE IF NOT EXISTS system_transfer_with_seed AS BASE_EVENTS
COMMENT 'System token transfer with seed';
ALTER TABLE system_transfer_with_seed
    ADD COLUMN IF NOT EXISTS source                  String,
    ADD COLUMN IF NOT EXISTS destination             String,
    ADD COLUMN IF NOT EXISTS lamports                UInt64,
    ADD COLUMN IF NOT EXISTS source_base             String COMMENT 'Base account address for the seed.',
    ADD COLUMN IF NOT EXISTS source_owner            String COMMENT 'Owner of the source account.',
    ADD COLUMN IF NOT EXISTS source_seed             String COMMENT 'Seed used to derive the source account.';

-- WithdrawNonceAccount --
CREATE TABLE IF NOT EXISTS system_withdraw_nonce_account AS BASE_EVENTS
COMMENT 'System token withdraw nonce account';
ALTER TABLE system_withdraw_nonce_account
    ADD COLUMN IF NOT EXISTS destination             String,
    ADD COLUMN IF NOT EXISTS lamports                UInt64,
    ADD COLUMN IF NOT EXISTS nonce_account           String COMMENT 'Nonce account address.',
    ADD COLUMN IF NOT EXISTS nonce_authority         String COMMENT 'Nonce authority account address.';
