use buffa::Message;
use dex_pools::{Creation, EvidencePosition, InstructionPosition, Pool, Projection, Protocol, Token};
use dex_swaps_pools::{map_pools_impl, pb, pool_entries, to_pool_creations, to_sink_entries, POOL_TYPE_URL};
use substreams::pb::substreams::store_delta::Operation;
use substreams::store::{DeltaProto, Deltas};

fn projection() -> Projection {
    Projection {
        slot: 350_000_026,
        blockhash: "test-block".into(),
        pools: vec![Pool {
            protocol: Protocol::PumpfunAmm,
            address: [1; 32],
            program_id: [2; 32],
            tokens: [
                Token {
                    mint: [3; 32],
                    vault: Some([4; 32]),
                    decimals: Some(0),
                    token_program: Some([5; 32]),
                },
                Token {
                    mint: [6; 32],
                    vault: None,
                    decimals: None,
                    token_program: None,
                },
            ],
            creation: Creation {
                slot: 350_000_026,
                blockhash: "test-block".into(),
                transaction_index: 7,
                signature: [8; 64],
                instruction: InstructionPosition {
                    instruction_index: 2,
                    inner_instruction_index: None,
                },
                evidence: EvidencePosition::Instruction(InstructionPosition {
                    instruction_index: 2,
                    inner_instruction_index: Some(0),
                }),
            },
        }],
    }
}

#[test]
fn entries_preserve_binary_keys_type_identity_presence_and_provenance() {
    let projection = projection();
    let entries = to_sink_entries(&projection);
    assert!(entries.if_not_exist);
    assert_eq!(entries.entries.len(), 1);
    let entry = &entries.entries[0];
    assert_eq!(entry.key.as_option().unwrap().bytes, [1; 32]);
    let any = entry.value.as_option().unwrap();
    assert_eq!(any.type_url, POOL_TYPE_URL);
    let creations = to_pool_creations(&projection);
    assert_eq!(pool_entries(&creations), entries);
    assert_eq!(any.value.as_ref(), creations.pools[0].encode_to_vec());
    let pool = pb::Pool::decode_from_slice(&any.value).unwrap();
    assert_eq!(pool.pool_address, [1; 32]);
    assert_eq!(pool.program_id, [2; 32]);
    assert_eq!(pool.protocol.as_known(), Some(pb::Protocol::PROTOCOL_PUMPFUN_AMM));
    assert_eq!(pool.tokens[0].mint, [3; 32]);
    assert_eq!(pool.tokens[0].decimals, Some(0));
    assert_eq!(pool.tokens[0].vault, Some(vec![4; 32]));
    assert_eq!(pool.tokens[0].token_program, Some(vec![5; 32]));
    assert_eq!(pool.tokens[1].mint, [6; 32]);
    assert_eq!(pool.tokens[1].decimals, None);
    assert_eq!(pool.tokens[1].vault, None);
    assert_eq!(pool.tokens[1].token_program, None);
    let creation = pool.creation.as_option().unwrap();
    assert_eq!(creation.slot, projection.slot);
    assert_eq!(creation.blockhash, projection.blockhash);
    assert_eq!(creation.input_transaction_index, 7);
    assert_eq!(creation.signature, [8; 64]);
    assert_eq!(creation.instruction.as_option().unwrap().inner_instruction_index, None);
    assert!(matches!(&creation.evidence, Some(pb::creation::Evidence::EventInstruction(p)) if p.inner_instruction_index == Some(0)));
    assert_eq!(entries.encode_to_vec(), to_sink_entries(&projection).encode_to_vec());
}

#[test]
fn log_zero_is_present_and_empty_blocks_are_writable() {
    let mut projection = projection();
    projection.pools[0].protocol = Protocol::OrcaWhirlpool;
    projection.pools[0].creation.evidence = EvidencePosition::Log { log_index: 0 };
    let entries = to_sink_entries(&projection);
    let pool = pb::Pool::decode_from_slice(&entries.entries[0].value.as_option().unwrap().value).unwrap();
    assert_eq!(pool.protocol.as_known(), Some(pb::Protocol::PROTOCOL_ORCA_WHIRLPOOL));
    assert!(matches!(pool.creation.as_option().unwrap().evidence, Some(pb::creation::Evidence::LogIndex(0))));
    projection.pools.clear();
    let entries = to_sink_entries(&projection);
    assert!(entries.entries.is_empty());
    assert!(entries.if_not_exist);
}

#[test]
fn legacy_delta_output_preserves_pool_payload_and_only_emits_creates() {
    let creations = to_pool_creations(&projection());
    let pool = &creations.pools[0];
    let delta = |operation| DeltaProto {
        operation,
        ordinal: 0,
        key: dex_pools::solana::base58::encode(&pool.pool_address),
        old_value: pb::Pool::default(),
        new_value: pool.clone(),
    };
    let output = map_pools_impl(Deltas {
        deltas: vec![delta(Operation::Update), delta(Operation::Create), delta(Operation::Delete)],
    });
    assert_eq!(output.encode_to_vec(), creations.encode_to_vec());
    let hosted = pool_entries(&creations);
    assert_eq!(output.pools[0].encode_to_vec(), hosted.entries[0].value.as_option().unwrap().value.as_ref());
    assert!(map_pools_impl(Deltas { deltas: vec![] }).pools.is_empty());
}
