CREATE TABLE IF NOT EXISTS base_events (
    -- block --
    block_num                   UInt32,
    block_hash                  String,
    timestamp                   DateTime(0, 'UTC'),

    -- ordering --
    transaction_index           UInt32,
    instruction_index           UInt32,

    -- transaction --
    signature                   String,
    signature_hash              UInt64  MATERIALIZED cityHash64(signature),
    fee_payer                   String,
    signers_raw                 String,
    signers                     Array(String) MATERIALIZED splitByChar(',', signers_raw),
    signer                      String MATERIALIZED if(length(signers) > 0, signers[1], ''),
    fee                         UInt64 DEFAULT 0,
    compute_units_consumed      UInt64 DEFAULT 0,

    -- instruction --
    program_id                  LowCardinality(String),
    stack_height                UInt32,

    -- indexes -
    INDEX idx_program_id        (program_id)        TYPE set(8)                 GRANULARITY 1,
    INDEX idx_fee_payer         (fee_payer)         TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_signature         (signature)         TYPE bloom_filter(0.005)    GRANULARITY 1,
    INDEX idx_signer            (signer)            TYPE bloom_filter(0.005)    GRANULARITY 1
)
ENGINE = ReplacingMergeTree
ORDER BY (
    timestamp, block_num,
    block_hash, transaction_index, instruction_index
);
