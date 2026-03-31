CREATE TABLE IF NOT EXISTS BASE_EVENTS (
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
    compute_units_consumed      UInt64 DEFAULT 0,

    -- instruction --
    program_id                  LowCardinality(String),
    stack_height                UInt32,

    -- indexes --
    INDEX idx_timestamp         (timestamp)         TYPE minmax                 GRANULARITY 1,
    INDEX idx_block_num         (block_num)         TYPE minmax                 GRANULARITY 1,

    -- count() --
    PROJECTION prj_fee_payer_count ( SELECT fee_payer, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY fee_payer ),
    PROJECTION prj_signer_count ( SELECT signer, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY signer ),
    PROJECTION prj_program_id_count ( SELECT program_id, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY program_id ),

    -- minute + timestamp --
    PROJECTION prj_signature_by_timestamp ( SELECT signature, minute, timestamp GROUP BY signature, minute, timestamp ),

    -- minute --
    PROJECTION prj_fee_payer_by_minute ( SELECT fee_payer, minute GROUP BY fee_payer, minute ),
    PROJECTION prj_signer_by_minute ( SELECT signer, minute GROUP BY signer, minute ),
    PROJECTION prj_program_id_by_minute ( SELECT program_id, minute GROUP BY program_id, minute )
)
ENGINE = MergeTree
ORDER BY (
    timestamp, block_num,
    transaction_index, instruction_index
);
