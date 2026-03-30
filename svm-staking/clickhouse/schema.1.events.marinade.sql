-- Marinade Deposit --
CREATE TABLE IF NOT EXISTS marinade_deposit AS base_events
COMMENT 'Marinade Finance deposit SOL → mSOL';
ALTER TABLE marinade_deposit
    ADD COLUMN IF NOT EXISTS state                          String COMMENT 'Marinade state account',
    ADD COLUMN IF NOT EXISTS sol_owner                      String COMMENT 'SOL depositor',
    ADD COLUMN IF NOT EXISTS sol_swapped                    UInt64 COMMENT 'SOL swapped via liquid pool',
    ADD COLUMN IF NOT EXISTS msol_swapped                   UInt64 COMMENT 'mSOL received from liquid pool',
    ADD COLUMN IF NOT EXISTS sol_deposited                  UInt64 COMMENT 'SOL deposited to stake',
    ADD COLUMN IF NOT EXISTS msol_minted                    UInt64 COMMENT 'Total mSOL minted',
    ADD COLUMN IF NOT EXISTS total_virtual_staked_lamports  UInt64 COMMENT 'Total virtual staked lamports after',
    ADD COLUMN IF NOT EXISTS msol_supply                    UInt64 COMMENT 'mSOL total supply after';

-- Marinade Deposit Stake Account --
CREATE TABLE IF NOT EXISTS marinade_deposit_stake_account AS base_events
COMMENT 'Marinade Finance deposit stake account → mSOL';
ALTER TABLE marinade_deposit_stake_account
    ADD COLUMN IF NOT EXISTS state                          String COMMENT 'Marinade state account',
    ADD COLUMN IF NOT EXISTS stake                          String COMMENT 'Stake account deposited',
    ADD COLUMN IF NOT EXISTS delegated                      UInt64 COMMENT 'Delegated lamports',
    ADD COLUMN IF NOT EXISTS withdrawer                     String COMMENT 'Stake withdrawer authority',
    ADD COLUMN IF NOT EXISTS validator                      String COMMENT 'Validator vote account',
    ADD COLUMN IF NOT EXISTS msol_minted                    UInt64 COMMENT 'mSOL minted',
    ADD COLUMN IF NOT EXISTS total_virtual_staked_lamports  UInt64 COMMENT 'Total virtual staked lamports after',
    ADD COLUMN IF NOT EXISTS msol_supply                    UInt64 COMMENT 'mSOL total supply after';

-- Marinade Liquid Unstake --
CREATE TABLE IF NOT EXISTS marinade_liquid_unstake AS base_events
COMMENT 'Marinade Finance liquid unstake mSOL → SOL';
ALTER TABLE marinade_liquid_unstake
    ADD COLUMN IF NOT EXISTS state              String COMMENT 'Marinade state account',
    ADD COLUMN IF NOT EXISTS msol_owner         String COMMENT 'mSOL owner',
    ADD COLUMN IF NOT EXISTS msol_amount        UInt64 COMMENT 'mSOL burned',
    ADD COLUMN IF NOT EXISTS msol_fee           UInt64 COMMENT 'mSOL fee',
    ADD COLUMN IF NOT EXISTS treasury_msol_cut  UInt64 COMMENT 'Treasury mSOL cut',
    ADD COLUMN IF NOT EXISTS sol_amount         UInt64 COMMENT 'SOL received';

-- Marinade Add Liquidity --
CREATE TABLE IF NOT EXISTS marinade_add_liquidity AS base_events
COMMENT 'Marinade Finance add SOL liquidity to SOL/mSOL pool';
ALTER TABLE marinade_add_liquidity
    ADD COLUMN IF NOT EXISTS state                          String COMMENT 'Marinade state account',
    ADD COLUMN IF NOT EXISTS sol_owner                      String COMMENT 'SOL provider',
    ADD COLUMN IF NOT EXISTS sol_added_amount               UInt64 COMMENT 'SOL added',
    ADD COLUMN IF NOT EXISTS lp_minted                      UInt64 COMMENT 'LP tokens minted',
    ADD COLUMN IF NOT EXISTS total_virtual_staked_lamports  UInt64 COMMENT 'Total virtual staked lamports after',
    ADD COLUMN IF NOT EXISTS msol_supply                    UInt64 COMMENT 'mSOL total supply after';

-- Marinade Remove Liquidity --
CREATE TABLE IF NOT EXISTS marinade_remove_liquidity AS base_events
COMMENT 'Marinade Finance remove liquidity (burn LP tokens)';
ALTER TABLE marinade_remove_liquidity
    ADD COLUMN IF NOT EXISTS state          String COMMENT 'Marinade state account',
    ADD COLUMN IF NOT EXISTS lp_burned      UInt64 COMMENT 'LP tokens burned',
    ADD COLUMN IF NOT EXISTS sol_out_amount UInt64 COMMENT 'SOL received',
    ADD COLUMN IF NOT EXISTS msol_out_amount UInt64 COMMENT 'mSOL received';

-- Marinade Withdraw Stake Account --
CREATE TABLE IF NOT EXISTS marinade_withdraw_stake_account AS base_events
COMMENT 'Marinade Finance withdraw stake account (burn mSOL)';
ALTER TABLE marinade_withdraw_stake_account
    ADD COLUMN IF NOT EXISTS state          String COMMENT 'Marinade state account',
    ADD COLUMN IF NOT EXISTS stake          String COMMENT 'Split stake account',
    ADD COLUMN IF NOT EXISTS validator      String COMMENT 'Validator vote account',
    ADD COLUMN IF NOT EXISTS user_msol_auth String COMMENT 'User mSOL authority',
    ADD COLUMN IF NOT EXISTS msol_burned    UInt64 COMMENT 'mSOL burned',
    ADD COLUMN IF NOT EXISTS msol_fees      UInt64 COMMENT 'mSOL fees',
    ADD COLUMN IF NOT EXISTS beneficiary    String COMMENT 'Beneficiary account',
    ADD COLUMN IF NOT EXISTS split_lamports UInt64 COMMENT 'Split stake lamports';
