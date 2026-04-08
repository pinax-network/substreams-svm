CREATE TABLE IF NOT EXISTS base_events (
    -- block --
    block_num                   UInt32,
    block_hash                  String,
    timestamp                   DateTime(0, 'UTC'),
    version                     UInt64  MATERIALIZED to_version(block_num, transaction_index, instruction_index),

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
    compute_units_consumed      UInt64 DEFAULT 0,

    -- instruction --
    program_id                  LowCardinality(String),
    stack_height                UInt32,

    -- indexes -
    INDEX idx_timestamp         (timestamp)         TYPE minmax                 GRANULARITY 1,
    INDEX idx_block_num         (block_num)         TYPE minmax                 GRANULARITY 1,
    INDEX idx_program_id        (program_id)        TYPE set(8)                 GRANULARITY 1,
    INDEX idx_fee_payer         (fee_payer)         TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_signature         (signature)         TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_signer            (signer)            TYPE bloom_filter(0.005)    GRANULARITY 1
)
ENGINE = ReplacingMergeTree
-- production tables are derived from MV's on these base tables
ORDER BY (
    timestamp, block_num,
    block_hash, transaction_index, instruction_index
);

ALTER TABLE base_events
  MODIFY SETTING deduplicate_merge_projection_mode = 'rebuild';

ALTER TABLE base_events
    ADD PROJECTION IF NOT EXISTS prj_signature (SELECT signature, timestamp, _part_offset ORDER BY (signature, timestamp)),
    ADD PROJECTION IF NOT EXISTS prj_fee_payer (SELECT fee_payer, timestamp, _part_offset ORDER BY (fee_payer, timestamp)),
    ADD PROJECTION IF NOT EXISTS prj_signer (SELECT signer, timestamp, _part_offset ORDER BY (signer, timestamp));

CREATE TABLE IF NOT EXISTS base_transactions AS base_events;
ALTER TABLE base_transactions
    DROP PROJECTION IF EXISTS prj_part_program_id,
    DROP INDEX IF EXISTS idx_program_id,
    DROP COLUMN IF EXISTS program_id,
    DROP COLUMN IF EXISTS stack_height;
