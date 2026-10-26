//! Execute the committed package graph, including the no-vote input, so native
//! tests cannot hide a stale WASM binary or incorrectly wired manifest.
#[path = "../../svm-transfers/tests/support/package_wasm.rs"]
mod package_wasm;

use buffa::Message;
use dex_pools::{solana::pb::sf::solana::r#type::v1 as solana, Block};
use dex_swaps_pools::{map_pool_entries_impl, pb, POOL_TYPE_URL};
use flate2::read::GzDecoder;
use package_wasm::{Package, Sources};
use std::{io::Read, path::Path};
use substreams::pb::sf::substreams::foundational_store::model::v2::SinkEntries;

fn package() -> Package {
    let package = Package::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../spkg/dex-swaps-pools-v0.1.0.spkg"));
    // Built with Rust 1.85.0, workspace feature unification and the path-remap
    // flags documented in docs/releasing.md. Never replace a released package.
    assert_eq!(package.sha256, "8feffabfeae4f3c5e3c37db6645dce02218f943c0e77399aa6fcda88564b0347");
    assert_eq!(
        package.binary_of("map_pool_entries"),
        "6e181abb1a106d53e7155b999aa0ac553357063fa1fc96367416994504d0b6c0"
    );
    package
}

fn fixture(bytes: &[u8]) -> Block {
    let mut decoded = Vec::new();
    GzDecoder::new(bytes).take(1024 * 1024).read_to_end(&mut decoded).unwrap();
    Block::decode_from_slice(&decoded).unwrap()
}

fn run(package: &Package, block: &Block) -> Result<Vec<u8>, String> {
    package.evaluate(
        "map_pool_entries",
        &Sources {
            block: &block.encode_to_vec(),
            clock: &[],
            params: &[],
        },
    )
}

#[test]
fn committed_package_runs_one_no_vote_input_and_matches_native_creation_records() {
    let package = package();
    assert_eq!(package.meta[0], ("dex_swaps_pools".to_owned(), "v0.1.0".to_owned()));
    assert_eq!(package.wiring("map_pool_entries", ""), "map_pool_entries <- [map map_pool_creations]");
    assert_eq!(
        package.wiring("map_pool_creations", ""),
        "map_pool_creations <- [map solana_common:blocks_without_votes]"
    );
    assert_eq!(package.wiring("store_pools", ""), "store_pools <- [map map_pool_creations]");
    for (bytes, protocol, expected_count) in [
        (
            include_bytes!("../../dex-pools/fixtures/pump-v2-349999876-selected.pb.gz").as_slice(),
            pb::Protocol::PROTOCOL_PUMPFUN_AMM,
            0,
        ),
        (
            include_bytes!("../../dex-pools/fixtures/pump-current-461766718-selected.pb.gz").as_slice(),
            pb::Protocol::PROTOCOL_PUMPFUN_AMM,
            1,
        ),
        (
            include_bytes!("../../dex-pools/fixtures/orca-v2-461667419-selected.pb.gz").as_slice(),
            pb::Protocol::PROTOCOL_ORCA_WHIRLPOOL,
            1,
        ),
    ] {
        let block = fixture(bytes);
        let native = map_pool_entries_impl(&block).unwrap();
        assert_eq!(native.entries.len(), expected_count);
        let encoded = run(&package, &block).expect("WASM must accept captured creation");
        assert_eq!(encoded, native.encode_to_vec(), "packaged WASM must match native extraction and encoding");
        let entries = SinkEntries::decode_from_slice(&encoded).unwrap();
        assert!(entries.if_not_exist);
        if expected_count == 0 {
            // The historical migration's complete transaction has truncated
            // logs: raw V2 decoding succeeds, but commitment is unproved.
            assert!(block.transactions[0].meta.log_messages.iter().any(|line| line == "Log truncated"));
            assert!(entries.entries.is_empty());
            continue;
        }
        let value = entries.entries[0].value.as_option().unwrap();
        assert_eq!(value.type_url, POOL_TYPE_URL);
        let pool = pb::Pool::decode_from_slice(&value.value).unwrap();
        assert_eq!(pool.protocol.as_known(), Some(protocol));
        assert_eq!(entries.entries[0].key.as_option().unwrap().bytes, pool.pool_address);

        let mut failed = block.clone();
        failed.transactions[0].meta.as_option_mut().unwrap().err = solana::TransactionError::default().into();
        assert!(SinkEntries::decode_from_slice(&run(&package, &failed).unwrap()).unwrap().entries.is_empty());
    }
}

#[test]
fn committed_package_preserves_empty_progress_and_rejects_malformed_blocks() {
    let package = package();
    let mut block = fixture(include_bytes!("../../dex-pools/fixtures/orca-v2-461667419-selected.pb.gz"));
    block.transactions.clear();
    let empty = SinkEntries::decode_from_slice(&run(&package, &block).unwrap()).unwrap();
    assert!(empty.entries.is_empty());
    assert!(empty.if_not_exist);
    let mut block = fixture(include_bytes!("../../dex-pools/fixtures/orca-v2-461667419-selected.pb.gz"));
    block.transactions[0].meta = Default::default();
    assert!(map_pool_entries_impl(&block).is_err());
    // The imported source omits transactions with missing metadata before our
    // mapper receives them. Test malformed retained input separately.
    assert!(SinkEntries::decode_from_slice(&run(&package, &block).unwrap()).unwrap().entries.is_empty());
    let mut malformed = fixture(include_bytes!("../../dex-pools/fixtures/orca-v2-461667419-selected.pb.gz"));
    malformed.transactions[0]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .account_keys[0]
        .pop();
    assert!(map_pool_entries_impl(&malformed).is_err());
    assert!(run(&package, &malformed).is_err());
}

#[test]
fn committed_no_vote_input_does_not_claim_original_transaction_indexes() {
    let package = package();
    let mut block = fixture(include_bytes!("../../dex-pools/fixtures/orca-v2-461667419-selected.pb.gz"));
    let vote = solana::ConfirmedTransaction {
        transaction: solana::Transaction {
            signatures: vec![vec![7; 64]],
            message: solana::Message {
                header: solana::MessageHeader {
                    num_required_signatures: 1,
                    ..Default::default()
                }
                .into(),
                account_keys: vec![
                    vec![7; 32],
                    dex_pools::solana::base58::decode("Vote111111111111111111111111111111111111111").unwrap(),
                ],
                recent_blockhash: vec![9; 32],
                instructions: vec![solana::CompiledInstruction {
                    program_id_index: 1,
                    ..Default::default()
                }],
                ..Default::default()
            }
            .into(),
        }
        .into(),
        meta: solana::TransactionStatusMeta::default().into(),
    };
    block.transactions.insert(0, vote);
    let unfiltered = dex_pools::project(&block).unwrap();
    assert_eq!(unfiltered.pools[0].creation.transaction_index, 1);
    let encoded = run(&package, &block).unwrap();
    let entries = SinkEntries::decode_from_slice(&encoded).unwrap();
    let stored = pb::Pool::decode_from_slice(&entries.entries[0].value.as_option().unwrap().value).unwrap();
    let creation = stored.creation.as_option().unwrap();
    assert_eq!(creation.input_transaction_index, 0);
    assert_eq!(creation.signature, unfiltered.pools[0].creation.signature);
}

#[test]
fn committed_legacy_store_and_hosted_entries_share_payloads_and_first_write_policy() {
    use std::collections::btree_map::Entry;
    use std::collections::BTreeMap;
    use substreams::pb::substreams::{store_delta::Operation, StoreDelta, StoreDeltas};

    let package = package();
    assert_eq!(package.wiring("map_pools", ""), "map_pools <- [store store_pools deltas]");
    let mut stored = BTreeMap::<String, Vec<u8>>::new();
    for bytes in [
        include_bytes!("../../dex-pools/fixtures/pump-current-461766718-selected.pb.gz").as_slice(),
        include_bytes!("../../dex-pools/fixtures/orca-v2-461667419-selected.pb.gz").as_slice(),
    ] {
        let block = fixture(bytes);
        let encoded_block = block.encode_to_vec();
        let decoded = package
            .evaluate(
                "map_pool_creations",
                &Sources {
                    block: &encoded_block,
                    clock: &[],
                    params: &[],
                },
            )
            .unwrap();
        let pools = pb::Pools::decode_from_slice(&decoded).unwrap();
        assert_eq!(pools.pools.len(), 1);
        let hosted = SinkEntries::decode_from_slice(&package.run("map_pool_entries", &[&decoded]).unwrap()).unwrap();
        assert!(hosted.if_not_exist);
        assert_eq!(hosted.entries.len(), 1);
        // Exercise the actual packed store handler. A bounded test store applies
        // only its declared set_if_not_exists operations and passes real encoded
        // StoreDeltas to the packed map_pools consumer. This is not a reorg test.
        for replay in [false, true] {
            let writes = package.run_store("store_pools", &[&decoded]).unwrap();
            assert_eq!(writes.len(), 1);
            let mut deltas = StoreDeltas::default();
            for write in writes {
                let entry = &hosted.entries[0];
                assert_eq!(dex_pools::solana::base58::decode(&write.key).unwrap(), entry.key.bytes);
                assert_eq!(write.value, entry.value.value);
                if let Entry::Vacant(slot) = stored.entry(write.key.clone()) {
                    slot.insert(write.value.clone());
                    deltas.store_deltas.push(StoreDelta {
                        operation: Operation::Create.into(),
                        ordinal: write.ordinal,
                        key: write.key,
                        old_value: Vec::new(),
                        new_value: write.value,
                    });
                }
            }
            if !replay {
                // The consumer contract is Create-only, even if presented with
                // other valid delta operations by a future producer.
                let mut update = deltas.store_deltas[0].clone();
                update.operation = Operation::Update.into();
                update.old_value = update.new_value.clone();
                let mut delete = update.clone();
                delete.operation = Operation::Delete.into();
                delete.new_value.clear();
                deltas.store_deltas.extend([update, delete]);
            }
            let output = package.run("map_pools", &[&deltas.encode_to_vec()]).unwrap();
            let created = pb::Pools::decode_from_slice(&output).unwrap();
            if replay {
                assert!(created.pools.is_empty(), "a repeated identity must not create a second legacy delta");
            } else {
                assert_eq!(created.encode_to_vec(), decoded, "both output branches must retain every pool field");
            }
        }
    }
    assert_eq!(stored.len(), 2);
}
