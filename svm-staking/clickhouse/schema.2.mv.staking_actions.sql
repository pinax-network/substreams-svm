-- Unified Staking Actions --
CREATE TABLE IF NOT EXISTS staking_actions AS base_events
COMMENT 'Unified staking actions across all protocols';
ALTER TABLE staking_actions
    ADD COLUMN IF NOT EXISTS protocol             LowCardinality(String) COMMENT 'Protocol name (native_stake, marinade)',
    ADD COLUMN IF NOT EXISTS action               LowCardinality(String) COMMENT 'Action type (stake, unstake, withdraw, add_liquidity)',
    ADD COLUMN IF NOT EXISTS account              String COMMENT 'User/owner account',
    ADD COLUMN IF NOT EXISTS amount               UInt64 DEFAULT 0 COMMENT 'Amount in lamports',
    ADD COLUMN IF NOT EXISTS validator            String COMMENT 'Validator vote account (if applicable)',

    -- indexes --
    ADD INDEX IF NOT EXISTS idx_protocol          (protocol)          TYPE set(8)                 GRANULARITY 1,
    ADD INDEX IF NOT EXISTS idx_action            (action)            TYPE set(8)                 GRANULARITY 1,
    ADD INDEX IF NOT EXISTS idx_account           (account)           TYPE bloom_filter(0.005)    GRANULARITY 1,
    ADD INDEX IF NOT EXISTS idx_validator         (validator)         TYPE bloom_filter(0.005)    GRANULARITY 1,
    ADD INDEX IF NOT EXISTS idx_amount            (amount)            TYPE minmax                 GRANULARITY 1;

/* ──────────────────────────────────────────────────────────────────────────
   Native Stake → staking_actions
   ────────────────────────────────────────────────────────────────────────── */

-- Delegate = stake
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_native_stake_delegate
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'native_stake'      AS protocol,
    'stake'             AS action,
    stake_authority     AS account,
    0                   AS amount,
    vote_account        AS validator
FROM native_stake_delegate;

-- Deactivate = unstake
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_native_stake_deactivate
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'native_stake'      AS protocol,
    'unstake'           AS action,
    stake_authority     AS account,
    0                   AS amount,
    ''                  AS validator
FROM native_stake_deactivate;

-- Withdraw = withdraw
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_native_stake_withdraw
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'native_stake'      AS protocol,
    'withdraw'          AS action,
    withdraw_authority  AS account,
    lamports            AS amount,
    ''                  AS validator
FROM native_stake_withdraw;

/* ──────────────────────────────────────────────────────────────────────────
   Marinade → staking_actions
   ────────────────────────────────────────────────────────────────────────── */

-- Deposit = stake
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_marinade_deposit_stake
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'marinade'          AS protocol,
    'stake'             AS action,
    sol_owner           AS account,
    sol_deposited       AS amount,
    ''                  AS validator
FROM marinade_deposit;

-- Deposit Stake Account = stake
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_marinade_deposit_stake_account_stake
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'marinade'          AS protocol,
    'stake'             AS action,
    withdrawer          AS account,
    delegated           AS amount,
    validator           AS validator
FROM marinade_deposit_stake_account;

-- Liquid Unstake = unstake
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_marinade_liquid_unstake
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'marinade'          AS protocol,
    'unstake'           AS action,
    msol_owner          AS account,
    sol_amount          AS amount,
    ''                  AS validator
FROM marinade_liquid_unstake;

-- Withdraw Stake Account = withdraw
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_marinade_withdraw_stake_account
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'marinade'          AS protocol,
    'withdraw'          AS action,
    user_msol_auth      AS account,
    split_lamports      AS amount,
    validator           AS validator
FROM marinade_withdraw_stake_account;

-- Add Liquidity = add_liquidity
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_marinade_add_liquidity
TO staking_actions AS
SELECT
    block_num, block_hash, timestamp,
    transaction_index, instruction_index,
    signature, fee_payer, signers_raw, fee, compute_units_consumed,
    program_id, stack_height,

    'marinade'          AS protocol,
    'add_liquidity'     AS action,
    sol_owner           AS account,
    sol_added_amount    AS amount,
    ''                  AS validator
FROM marinade_add_liquidity;
