-- DumpFun Buy --
CREATE TABLE IF NOT EXISTS dumpfun_buy AS BASE_EVENTS
COMMENT 'DumpFun Buy';
ALTER TABLE dumpfun_buy
    ADD COLUMN IF NOT EXISTS user               String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS mint               String COMMENT 'Token mint',
    ADD COLUMN IF NOT EXISTS sol_in             UInt64 COMMENT 'SOL amount in',
    ADD COLUMN IF NOT EXISTS token_out          UInt64 COMMENT 'Token amount out',
    ADD COLUMN IF NOT EXISTS buy_time           Int64 COMMENT 'Buy timestamp';

-- DumpFun Sell --
CREATE TABLE IF NOT EXISTS dumpfun_sell AS BASE_EVENTS
COMMENT 'DumpFun Sell';
ALTER TABLE dumpfun_sell
    ADD COLUMN IF NOT EXISTS user               String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS mint               String COMMENT 'Token mint',
    ADD COLUMN IF NOT EXISTS token_in           UInt64 COMMENT 'Token amount in',
    ADD COLUMN IF NOT EXISTS sol_out            UInt64 COMMENT 'SOL amount out',
    ADD COLUMN IF NOT EXISTS sell_time          Int64 COMMENT 'Sell timestamp';
