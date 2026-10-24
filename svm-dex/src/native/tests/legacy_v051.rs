use super::*;
use crate::pb;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[path = "../../../tests/support/legacy_wasm.rs"]
mod legacy_wasm;

fn rows(changes: DatabaseChanges) -> Value {
    json!(changes
        .table_changes
        .into_iter()
        .filter(|row| row.table == "swaps")
        .map(|row| {
            let fields: BTreeMap<_, _> = row.fields.into_iter().map(|field| (field.name, field.value)).collect();
            json!({"table":row.table,"ordinal":row.ordinal,"fields":fields})
        })
        .collect::<Vec<_>>())
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}
fn unhex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn synthetic() -> Vec<(String, pb::Events)> {
    let swap = pb::Swap {
        protocol: pb::Protocol::MeteoraDlmm.into(),
        program_id: vec![1; 32],
        amm: vec![2; 32],
        amm_pool: vec![3; 32],
        user: vec![4; 32],
        input_mint: vec![5; 32],
        output_mint: vec![6; 32],
        input_amount: u64::MAX,
        output_amount: u64::MAX - 1,
        stack_height: 2,
    };
    let tx = pb::Transaction {
        signature: vec![7; 64],
        fee_payer: vec![8; 32],
        signers: vec![vec![8; 32], vec![9; 32]],
        fee: u64::MAX,
        compute_units_consumed: u64::MAX,
        swaps: vec![swap.clone()],
    };
    let events = |tx: pb::Transaction| pb::Events { transactions: vec![tx] };
    let mut cases = vec![
        ("empty_events".into(), pb::Events::default()),
        ("large_amounts_signers".into(), events(tx.clone())),
    ];
    for protocol in 0..=21 {
        cases.push((
            format!("protocol_{protocol}"),
            events(pb::Transaction {
                swaps: vec![pb::Swap {
                    protocol: protocol.into(),
                    ..swap.clone()
                }],
                ..tx.clone()
            }),
        ));
    }
    cases.push((
        "unknown_protocol".into(),
        events(pb::Transaction {
            swaps: vec![pb::Swap {
                protocol: 999.into(),
                ..swap.clone()
            }],
            ..tx.clone()
        }),
    ));
    cases.push((
        "empty_fields".into(),
        events(pb::Transaction {
            swaps: vec![pb::Swap::default()],
            ..Default::default()
        }),
    ));
    cases.push((
        "dense_transactions_and_row_order".into(),
        pb::Events {
            transactions: vec![
                pb::Transaction { swaps: vec![], ..tx.clone() },
                pb::Transaction {
                    swaps: vec![
                        pb::Swap {
                            protocol: pb::Protocol::JupiterV6.into(),
                            ..swap.clone()
                        },
                        swap,
                    ],
                    ..tx.clone()
                },
                tx,
            ],
        },
    ));
    cases
}
#[test]
fn released_v051_formatter_and_complete_block_parity() {
    let oracle = legacy_wasm::Oracle::new();
    let source = fixture();
    let clock = clock(&source);
    let mut corpus = vec![];
    for (name, events) in synthetic() {
        let input = events.encode_to_vec();
        let output = oracle.format(&clock.encode_to_vec(), &input).expect("historical formatter panicked");
        let old = rows(DatabaseChanges::decode_from_slice(&output).unwrap());
        let native = rows(crate::changes(&clock, &events).unwrap());
        assert_eq!(native, old, "formatter case {name}");
        corpus.push(json!({"name":name,"events_hex":hex(&input),"oracle_database_changes_hex":hex(&output),"rows":old}));
    }
    let bytes = source.encode_to_vec();
    let old_events = oracle.events(&bytes).expect("historical block projector panicked");
    let old = oracle.format(&clock.encode_to_vec(), &old_events).unwrap();
    let projection = dex_swaps::project(&source).unwrap();
    let actual = rows(changes(&clock, projection, 100_000, 64 * 1024 * 1024).unwrap());
    let expected = rows(DatabaseChanges::decode_from_slice(&old).unwrap());
    assert_eq!(actual, expected, "complete mainnet block");
    assert_eq!(actual.as_array().unwrap().len(), 231);
    let mut empty = source.clone();
    empty.transactions.clear();
    let empty_events = oracle.events(&empty.encode_to_vec()).unwrap();
    let empty_rows = oracle.format(&clock.encode_to_vec(), &empty_events).unwrap();
    assert_eq!(rows(DatabaseChanges::decode_from_slice(&empty_rows).unwrap()), json!([]));
    let result = json!({"schema_version":1,"package_sha256":"0774a2330e9d070d55f1e39d0b0c057415aaafec7df94602ac21671020c5bf46",
        "clock_hex":hex(&clock.encode_to_vec()),"cases":corpus,
        "complete_block":{"fixture":"dex-swaps/fixtures/solana-mainnet-350000026.pb.gz",
        "block_sha256":hex(&Sha256::digest(&bytes)),"oracle_events_hex":hex(&old_events),
        "oracle_database_changes_hex":hex(&old),"rows":expected}});
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy-v0.5.1/corpus.json");
    if std::env::var_os("UPDATE_SVM_DEX_V051_CORPUS").as_deref() == Some(std::ffi::OsStr::new("1")) {
        std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&result).unwrap())).unwrap();
    }
    let frozen: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // Encoded field/PK map order is not a database row contract. Retain the raw
    // WASM bytes in the frozen corpus and compare their decoded values/order.
    assert_eq!(frozen["package_sha256"], result["package_sha256"]);
    assert_eq!(frozen["clock_hex"], result["clock_hex"]);
    assert_eq!(frozen["cases"].as_array().unwrap().len(), result["cases"].as_array().unwrap().len());
    for (saved, current) in frozen["cases"].as_array().unwrap().iter().zip(result["cases"].as_array().unwrap()) {
        assert_eq!(saved["name"], current["name"]);
        assert_eq!(saved["events_hex"], current["events_hex"]);
        let saved_rows = rows(DatabaseChanges::decode_from_slice(&unhex(saved["oracle_database_changes_hex"].as_str().unwrap())).unwrap());
        assert_eq!(saved_rows, current["rows"]);
        assert_eq!(saved["rows"], current["rows"]);
    }
    assert_eq!(frozen["complete_block"]["block_sha256"], result["complete_block"]["block_sha256"]);
    assert_eq!(frozen["complete_block"]["oracle_events_hex"], result["complete_block"]["oracle_events_hex"]);
    assert_eq!(frozen["complete_block"]["rows"], result["complete_block"]["rows"]);
    assert_eq!(
        rows(DatabaseChanges::decode_from_slice(&unhex(frozen["complete_block"]["oracle_database_changes_hex"].as_str().unwrap())).unwrap()),
        result["complete_block"]["rows"]
    );
}
