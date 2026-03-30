-- Native Stake Initialize --
CREATE TABLE IF NOT EXISTS native_stake_initialize AS base_events
COMMENT 'Native Stake Program: initialize stake account';
ALTER TABLE native_stake_initialize
    ADD COLUMN IF NOT EXISTS is_root              Bool COMMENT 'Is root instruction',
    ADD COLUMN IF NOT EXISTS stake_account        String COMMENT 'Stake account',
    ADD COLUMN IF NOT EXISTS staker               String COMMENT 'Staker authority',
    ADD COLUMN IF NOT EXISTS withdrawer           String COMMENT 'Withdrawer authority',
    ADD COLUMN IF NOT EXISTS lockup_unix_timestamp Int64 DEFAULT 0 COMMENT 'Lockup timestamp',
    ADD COLUMN IF NOT EXISTS lockup_epoch         UInt64 DEFAULT 0 COMMENT 'Lockup epoch',
    ADD COLUMN IF NOT EXISTS lockup_custodian     String COMMENT 'Lockup custodian';

-- Native Stake Delegate --
CREATE TABLE IF NOT EXISTS native_stake_delegate AS base_events
COMMENT 'Native Stake Program: delegate stake to validator';
ALTER TABLE native_stake_delegate
    ADD COLUMN IF NOT EXISTS is_root              Bool COMMENT 'Is root instruction',
    ADD COLUMN IF NOT EXISTS stake_account        String COMMENT 'Stake account',
    ADD COLUMN IF NOT EXISTS vote_account         String COMMENT 'Validator vote account',
    ADD COLUMN IF NOT EXISTS stake_authority      String COMMENT 'Stake authority';

-- Native Stake Deactivate --
CREATE TABLE IF NOT EXISTS native_stake_deactivate AS base_events
COMMENT 'Native Stake Program: deactivate stake';
ALTER TABLE native_stake_deactivate
    ADD COLUMN IF NOT EXISTS is_root              Bool COMMENT 'Is root instruction',
    ADD COLUMN IF NOT EXISTS stake_account        String COMMENT 'Stake account',
    ADD COLUMN IF NOT EXISTS stake_authority      String COMMENT 'Stake authority';

-- Native Stake Withdraw --
CREATE TABLE IF NOT EXISTS native_stake_withdraw AS base_events
COMMENT 'Native Stake Program: withdraw from stake account';
ALTER TABLE native_stake_withdraw
    ADD COLUMN IF NOT EXISTS is_root              Bool COMMENT 'Is root instruction',
    ADD COLUMN IF NOT EXISTS stake_account        String COMMENT 'Stake account',
    ADD COLUMN IF NOT EXISTS destination          String COMMENT 'Destination account',
    ADD COLUMN IF NOT EXISTS lamports             UInt64 COMMENT 'Amount to withdraw',
    ADD COLUMN IF NOT EXISTS withdraw_authority   String COMMENT 'Withdraw authority',
    ADD COLUMN IF NOT EXISTS custodian            String COMMENT 'Lockup custodian';

-- Native Stake Merge --
CREATE TABLE IF NOT EXISTS native_stake_merge AS base_events
COMMENT 'Native Stake Program: merge two stake accounts';
ALTER TABLE native_stake_merge
    ADD COLUMN IF NOT EXISTS is_root                      Bool COMMENT 'Is root instruction',
    ADD COLUMN IF NOT EXISTS destination_stake_account     String COMMENT 'Destination stake account',
    ADD COLUMN IF NOT EXISTS source_stake_account          String COMMENT 'Source stake account',
    ADD COLUMN IF NOT EXISTS stake_authority               String COMMENT 'Stake authority';

-- Native Stake Split --
CREATE TABLE IF NOT EXISTS native_stake_split AS base_events
COMMENT 'Native Stake Program: split stake account';
ALTER TABLE native_stake_split
    ADD COLUMN IF NOT EXISTS is_root              Bool COMMENT 'Is root instruction',
    ADD COLUMN IF NOT EXISTS stake_account        String COMMENT 'Source stake account',
    ADD COLUMN IF NOT EXISTS split_stake_account  String COMMENT 'New split stake account',
    ADD COLUMN IF NOT EXISTS lamports             UInt64 COMMENT 'Amount to split',
    ADD COLUMN IF NOT EXISTS stake_authority      String COMMENT 'Stake authority';
