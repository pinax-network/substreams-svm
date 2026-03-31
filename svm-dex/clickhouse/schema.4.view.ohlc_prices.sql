CREATE VIEW IF NOT EXISTS ohlc_prices AS
SELECT
    -- bar interval --
    timestamp,
    interval_min,

    -- timestamp & block number --
    min(min_timestamp) as min_timestamp,
    max(max_timestamp) as max_timestamp,
    min(min_block_num) as min_block_num,
    max(max_block_num) as max_block_num,

    -- DEX identity --
    program_id,
    amm,
    amm_pool,
    mint0,
    mint1,

    -- Aggregate --
    argMinMerge(open0) AS open0,
    quantileDeterministicMerge(0.95)(quantile0) as high_quantile0,
    quantileDeterministicMerge(0.05)(quantile0) as low_quantile0,
    argMaxMerge(close0) AS close0,

    -- volume --
    sum(gross_volume0) AS gross_volume0,
    sum(gross_volume1) AS gross_volume1,
    sum(net_flow0) AS net_flow0,
    sum(net_flow1) AS net_flow1,

    -- universal --
    sum(transactions) as transactions
FROM state_ohlc_prices
GROUP BY
    interval_min,
    program_id, amm, amm_pool, mint0, mint1,
    timestamp;

CREATE VIEW IF NOT EXISTS ohlc_prices_uaw AS
SELECT
    -- bar interval --
    timestamp,
    interval_min,

    -- timestamp & block number --
    min(min_timestamp) as min_timestamp,
    max(max_timestamp) as max_timestamp,
    min(min_block_num) as min_block_num,
    max(max_block_num) as max_block_num,

    -- DEX identity --
    program_id,
    amm,
    amm_pool,
    mint0,
    mint1,

    -- Aggregate --
    argMinMerge(open0) AS open0,
    quantileDeterministicMerge(0.95)(quantile0) as high_quantile0,
    quantileDeterministicMerge(0.05)(quantile0) as low_quantile0,
    argMaxMerge(close0) AS close0,

    -- volume --
    sum(gross_volume0) AS gross_volume0,
    sum(gross_volume1) AS gross_volume1,
    sum(net_flow0) AS net_flow0,
    sum(net_flow1) AS net_flow1,

    -- universal with UAW fields --
    sum(transactions) as transactions,
    uniqMerge(uniq_signer) AS uaw_signer,
    uniqMerge(uniq_fee_payer) AS uaw_fee_payer,
    uniqMerge(uniq_user) AS uaw_user
FROM state_ohlc_prices
GROUP BY
    interval_min,
    program_id, amm, amm_pool, mint0, mint1,
    timestamp;