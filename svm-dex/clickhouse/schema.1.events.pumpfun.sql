-- Pump.fun Buy --
CREATE TABLE IF NOT EXISTS pumpfun_buy AS base_events
COMMENT 'Pump.fun Bonding Curve';
ALTER TABLE pumpfun_buy
    -- accounts --
    ADD COLUMN IF NOT EXISTS global String COMMENT 'Global config account',
    ADD COLUMN IF NOT EXISTS fee_recipient String COMMENT 'Fee recipient account',
    ADD COLUMN IF NOT EXISTS mint String COMMENT 'Token mint address',
    ADD COLUMN IF NOT EXISTS bonding_curve String COMMENT 'Bonding curve account',
    ADD COLUMN IF NOT EXISTS associated_bonding_curve String COMMENT 'Associated bonding curve account',
    ADD COLUMN IF NOT EXISTS associated_user String COMMENT 'Associated user account',
    ADD COLUMN IF NOT EXISTS user String COMMENT 'User account',
    ADD COLUMN IF NOT EXISTS creator_vault String COMMENT 'Creator vault account',

    -- data --
    ADD COLUMN IF NOT EXISTS amount UInt64 COMMENT 'Amount of tokens to buy',
    ADD COLUMN IF NOT EXISTS max_sol_cost UInt64 COMMENT 'Maximum amount of SOL to spend',

    -- event --
    ADD COLUMN IF NOT EXISTS sol_amount UInt64 COMMENT 'Amount of SOL spent',
    ADD COLUMN IF NOT EXISTS token_amount UInt64 COMMENT 'Amount of tokens received',
    ADD COLUMN IF NOT EXISTS is_buy Bool COMMENT 'True if buy, false if sell',
    ADD COLUMN IF NOT EXISTS virtual_sol_reserves UInt64 COMMENT 'Virtual SOL reserves',
    ADD COLUMN IF NOT EXISTS virtual_token_reserves UInt64 COMMENT 'Virtual token reserves',
    ADD COLUMN IF NOT EXISTS real_sol_reserves UInt64 DEFAULT 0 COMMENT 'Real SOL reserves',
    ADD COLUMN IF NOT EXISTS real_token_reserves UInt64 DEFAULT 0 COMMENT 'Real token reserves',
    ADD COLUMN IF NOT EXISTS protocol_fee_recipient String DEFAULT '' COMMENT 'Protocol fee recipient account',
    ADD COLUMN IF NOT EXISTS protocol_fee_basis_points UInt64 DEFAULT 0 COMMENT 'Protocol fee basis points (1 bp = 0.01 %)',
    ADD COLUMN IF NOT EXISTS protocol_fee UInt64 DEFAULT 0 COMMENT 'Protocol fee in lamports',
    ADD COLUMN IF NOT EXISTS creator String DEFAULT '' COMMENT 'Creator account',
    ADD COLUMN IF NOT EXISTS creator_fee_basis_points UInt64 DEFAULT 0 COMMENT 'Creator fee basis points (1 bp = 0.01 %)',
    ADD COLUMN IF NOT EXISTS creator_fee UInt64 DEFAULT 0 COMMENT 'Creator fee in lamports';

-- Pump.fun Bonding Curve Sell --
CREATE TABLE IF NOT EXISTS pumpfun_sell AS pumpfun_buy;
ALTER TABLE pumpfun_sell
    RENAME COLUMN IF EXISTS max_sol_cost TO min_sol_output;

