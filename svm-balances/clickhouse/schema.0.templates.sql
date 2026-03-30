CREATE TABLE IF NOT EXISTS BASE_TRANSACTIONS (
    -- block --
    block_num                   UInt32,
    block_hash                  String,
    timestamp                   DateTime(0, 'UTC'),
    minute                      UInt32 MATERIALIZED toRelativeMinuteNum(timestamp),

    -- ordering --
    transaction_index           UInt32,
    instruction_index           UInt32,

    -- transaction --
    signature                   String,
    fee_payer                   String,
    signers_raw                 String,
    signers                     Array(String) MATERIALIZED string_to_array(signers_raw),
    signer                      String MATERIALIZED if(length(signers) > 0, signers[1], ''),
    fee                         UInt64 DEFAULT 0,
    compute_units_consumed      UInt64 DEFAULT 0

    -- -- indexes --
    -- INDEX idx_timestamp         (timestamp)         TYPE minmax                 GRANULARITY 1,
    -- INDEX idx_block_num         (block_num)         TYPE minmax                 GRANULARITY 1,

    -- -- projections --
    -- PROJECTION prj_signature (SELECT signature, timestamp, _part_offset ORDER BY (signature, timestamp)),
    -- PROJECTION prj_fee_payer (SELECT fee_payer, timestamp, _part_offset ORDER BY (fee_payer, timestamp)),
    -- PROJECTION prj_signer (SELECT signer, timestamp, _part_offset ORDER BY (signer, timestamp))
)
ENGINE = ReplacingMergeTree
-- TTL to automatically clean up old data
-- production tables are derived from MV's on these base tables
TTL timestamp + INTERVAL 1 DAY
ORDER BY (
    timestamp, block_num,
    block_hash, transaction_index, instruction_index
)
SETTINGS deduplicate_merge_projection_mode = 'rebuild';
