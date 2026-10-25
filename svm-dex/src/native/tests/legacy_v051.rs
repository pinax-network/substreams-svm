use super::*;
use crate::pb;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use substreams_solana::base58;
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
/// One real transaction in a Firehose-format block of its own.
struct BlockCase {
    name: &'static str,
    difference: &'static str,
    source: &'static str,
    block: sol::Block,
    /// The unmodified RPC-derived transaction, before restoring its lookup.
    rpc_block: Option<sol::Block>,
}

fn single_transaction_block(slot: u64, block_time: i64, tag: u8, transaction: sol::ConfirmedTransaction) -> sol::Block {
    sol::Block {
        slot,
        parent_slot: slot - 1,
        blockhash: base58::encode([tag; 32]),
        previous_blockhash: base58::encode([tag + 1; 32]),
        block_time: sol::UnixTimestamp { timestamp: block_time }.into(),
        transactions: vec![transaction],
        ..Default::default()
    }
}

fn block_cases() -> Vec<BlockCase> {
    let mut cases = Vec::new();
    // `getTransaction` (JSON) fixtures from 2026 keep the resolved loaded
    // addresses but not the message's address table lookups, which a Firehose
    // block always carries with them and native validation counts. Restore one
    // placeholder lookup with the same index counts. Neither pipeline reads
    // lookup keys or indexes; the test checks that it changes no output.
    for (name, difference, source, slot, block_time, bytes) in [
        (
            "rpc_2026_orca_multihop",
            "native_orca_events_matched_to_pool",
            "dex-swaps/fixtures/aqua-orca-multihop.transaction.pb",
            454_558_265,
            1_791_466_115,
            include_bytes!("../../../../dex-swaps/fixtures/aqua-orca-multihop.transaction.pb").as_slice(),
        ),
        (
            "rpc_2026_meteora_amm_temporary_account",
            "native_restores_meteora_amm_leg",
            "dex-swaps/fixtures/knet.transaction.pb",
            452_949_660,
            1_791_034_822,
            include_bytes!("../../../../dex-swaps/fixtures/knet.transaction.pb").as_slice(),
        ),
        (
            "rpc_2026_pumpswap_tracking_flag",
            "native_restores_pumpswap_leg",
            "dex-swaps/fixtures/oilinu.transaction.pb",
            452_919_823,
            1_791_026_854,
            include_bytes!("../../../../dex-swaps/fixtures/oilinu.transaction.pb").as_slice(),
        ),
        (
            "rpc_2026_damm_v2_swap2",
            "native_restores_damm_v2_leg",
            "dex-swaps/fixtures/privacy.transaction.pb",
            453_196_517,
            1_791_100_854,
            include_bytes!("../../../../dex-swaps/fixtures/privacy.transaction.pb").as_slice(),
        ),
    ] {
        let original = sol::ConfirmedTransaction::decode_from_slice(bytes).unwrap();
        let mut restored = original.clone();
        let meta = restored.meta.as_option().unwrap();
        let (writable, readonly) = (meta.loaded_writable_addresses.len() as u8, meta.loaded_readonly_addresses.len() as u8);
        let message = restored.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        assert!(message.versioned && message.address_table_lookups.is_empty());
        message.address_table_lookups.push(sol::MessageAddressTableLookup {
            account_key: vec![255; 32],
            writable_indexes: (0..writable).collect(),
            readonly_indexes: (writable..writable + readonly).collect(),
        });
        let tag = 16 + 2 * cases.len() as u8;
        cases.push(BlockCase {
            name,
            difference,
            source,
            block: single_transaction_block(slot, block_time, tag, restored),
            rpc_block: Some(single_transaction_block(slot, block_time, tag, original)),
        });
    }
    // v0.5.1 drops any transaction with a static Vote key before extraction.
    // Native keeps it: append the key, read-only and unsigned, to a real swap.
    let source = fixture();
    let projection = dex_swaps::project(&source).unwrap();
    let index = projection
        .transactions
        .iter()
        .map(|tx| tx.transaction_index as usize)
        .find(|&index| source.transactions[index].transaction.message.address_table_lookups.is_empty())
        .unwrap();
    let mut block = source.clone();
    block.transactions = vec![source.transactions[index].clone()];
    cases.push(BlockCase {
        name: "mainnet_swap_transaction",
        difference: "none",
        source: "dex-swaps/fixtures/solana-mainnet-350000026.pb.gz",
        block: block.clone(),
        rpc_block: None,
    });
    let tx = &mut block.transactions[0];
    let message = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message
        .account_keys
        .push(base58::decode("Vote111111111111111111111111111111111111111").unwrap());
    message.header.as_option_mut().unwrap().num_readonly_unsigned_accounts += 1;
    let meta = tx.meta.as_option_mut().unwrap();
    meta.pre_balances.push(1);
    meta.post_balances.push(1);
    cases.push(BlockCase {
        name: "mainnet_swap_transaction_with_vote_key",
        difference: "native_keeps_vote_key_transaction",
        source: "dex-swaps/fixtures/solana-mainnet-350000026.pb.gz",
        block,
        rpc_block: None,
    });
    cases
}

/// `protocol pool input->output` for each row, in order.
fn legs(rows: &Value) -> Vec<String> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let field = |name: &str| row["fields"][name].as_str().unwrap().to_string();
            format!(
                "{} {} {}->{}",
                field("protocol"),
                field("amm_pool"),
                field("input_amount"),
                field("output_amount")
            )
        })
        .collect()
}

/// Accepted differences, (v0.5.1 legs, native legs). Native is the correction.
fn expected_legs(name: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    match name {
        // v0.5.1 shifts the first two events onto the next pools and drops
        // the real third hop (#224).
        "rpc_2026_orca_multihop" => (
            vec![
                "orca_whirlpool GNzK4LP1nbfFcMNwB1zG4htCkToXjKYn5mn5yVKvaExm 273199007->53097662110",
                "orca_whirlpool 9TT3NbWogWjbQh7b3s1hfcqYF7VJHmg8uNcXDjtPRnh4 53097662110->2214641291345",
            ],
            vec![
                "orca_whirlpool FVB6knS78TJJswCrBhhB2T3cUKBQ8XyRvqwsjDq3oWwE 273199007->53097662110",
                "orca_whirlpool GNzK4LP1nbfFcMNwB1zG4htCkToXjKYn5mn5yVKvaExm 53097662110->2214641291345",
                "orca_whirlpool 9TT3NbWogWjbQh7b3s1hfcqYF7VJHmg8uNcXDjtPRnh4 2170348465518->276039217",
            ],
        ),
        // The WSOL destination is created and closed in the transaction (#223).
        "rpc_2026_meteora_amm_temporary_account" => (
            vec![],
            vec!["meteora_amm B1JkXQH1yvTQtRavmAStwoy2Pu54SsjLqfWphL9VwpGu 999999999343109->773859208"],
        ),
        // PumpSwap `buy` with the volume-tracking flag (#223).
        "rpc_2026_pumpswap_tracking_flag" => (vec![], vec!["pumpfun_amm 9Bvmg9W8yFv3bKAWeCBNWwKtCNYi8BgKqGKTDtsrRPVx 55229629->576275924281"]),
        // DAMM v2 `swap2` with its mode byte and `EvtSwap2` (v0.5.2, #223);
        // the Raydium leg is unchanged.
        "rpc_2026_damm_v2_swap2" => (
            vec!["raydium_cpmm fWSGGqSA1NvHGiGNHj5tQSD4Vmw5cy8TEa2FUd1yE6h 826289746015025->131738879"],
            vec![
                "meteora_daam 2YwxAtr1XjYHm7VkraEVj4P4n7YL4m7NDsLjf4pUhZwX 1087808068->15609208729942477",
                "raydium_cpmm fWSGGqSA1NvHGiGNHj5tQSD4Vmw5cy8TEa2FUd1yE6h 826289746015025->131738879",
            ],
        ),
        "mainnet_swap_transaction_with_vote_key" => (vec![], vec!["pumpfun_amm FtXG452GP7jfZYNG5W6ryTE3WLJxv8CMPFyWW3Ls1Xru 17560685865->32143636"]),
        other => panic!("unreviewed block case: {other}"),
    }
}

fn block_case(oracle: &legacy_wasm::Oracle, case: &BlockCase) -> Value {
    let bytes = case.block.encode_to_vec();
    let clock = clock(&case.block);
    let old_events = oracle.events(&bytes).expect("historical block projector panicked");
    let old = oracle.format(&clock.encode_to_vec(), &old_events).unwrap();
    let oracle_rows = rows(DatabaseChanges::decode_from_slice(&old).unwrap());
    let native_rows = rows(changes(&clock, dex_swaps::project(&case.block).unwrap(), 100_000, 64 * 1024 * 1024).unwrap());
    if let Some(original) = &case.rpc_block {
        // The restored lookup is inert: v0.5.1 extracts the same events without
        // it, and native rows equal the current wrapper's on the original.
        assert_eq!(oracle.events(&original.encode_to_vec()).unwrap(), old_events, "{}", case.name);
        let current = crate::changes(&clock, &dex_swaps::map_events(original.clone()).unwrap()).unwrap();
        assert_eq!(rows(current), native_rows, "{}", case.name);
    }
    let equal = native_rows == oracle_rows;
    assert_eq!(equal, case.difference == "none", "unclassified block difference: {}", case.name);
    if !equal {
        let (old_legs, new_legs) = expected_legs(case.name);
        assert_eq!(legs(&oracle_rows), old_legs, "{}", case.name);
        assert_eq!(legs(&native_rows), new_legs, "{}", case.name);
    }
    json!({"name":case.name,"difference":case.difference,"source":case.source,
        "reconstructed":case.rpc_block.as_ref().map(|_| "address_table_lookups"),
        "block_sha256":hex(&Sha256::digest(&bytes)),"block_hex":hex(&bytes),"clock_hex":hex(&clock.encode_to_vec()),
        "oracle_events_hex":hex(&old_events),"oracle_database_changes_hex":hex(&old),
        "oracle_rows":oracle_rows,"native_rows":native_rows,"equal_rows":equal})
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
    let cases = block_cases();
    let block_cases: Vec<_> = cases.iter().map(|case| block_case(&oracle, case)).collect();
    // The vote-key copy keeps exactly the rows of the unmodified transaction.
    assert_eq!(block_cases[5]["native_rows"], block_cases[4]["native_rows"]);
    let result = json!({"schema_version":1,"package_sha256":"0774a2330e9d070d55f1e39d0b0c057415aaafec7df94602ac21671020c5bf46",
        "clock_hex":hex(&clock.encode_to_vec()),"cases":corpus,
        "complete_block":{"fixture":"dex-swaps/fixtures/solana-mainnet-350000026.pb.gz",
        "block_sha256":hex(&Sha256::digest(&bytes)),"oracle_events_hex":hex(&old_events),
        "oracle_database_changes_hex":hex(&old),"rows":expected},
        "block_cases":block_cases});
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
    let (saved, current) = (frozen["block_cases"].as_array().unwrap(), result["block_cases"].as_array().unwrap());
    assert_eq!(saved.len(), current.len());
    for (saved, current) in saved.iter().zip(current) {
        for field in [
            "name",
            "difference",
            "source",
            "reconstructed",
            "block_sha256",
            "block_hex",
            "clock_hex",
            "oracle_events_hex",
            "oracle_rows",
            "native_rows",
            "equal_rows",
        ] {
            assert_eq!(saved[field], current[field], "{} {field}", current["name"]);
        }
        let saved_rows = rows(DatabaseChanges::decode_from_slice(&unhex(saved["oracle_database_changes_hex"].as_str().unwrap())).unwrap());
        assert_eq!(saved_rows, current["oracle_rows"]);
    }
}

/// Buy instructions with the trailing `track_volume` flag (or Pump.fun's
/// `buy_exact_sol_in`), which v0.5.1 cannot decode.
fn current_buys(tx: &sol::ConfirmedTransaction, program: &str, flagged: [[u8; 8]; 2], any_length: Option<[u8; 8]>) -> usize {
    tx.walk_instructions()
        .filter(|instruction| base58::encode(instruction.program_id().0) == program && instruction.data().len() >= 8)
        .filter(|instruction| {
            let discriminator: [u8; 8] = instruction.data()[..8].try_into().unwrap();
            (instruction.data().len() == 25 && flagged.contains(&discriminator)) || any_length == Some(discriminator)
        })
        .count()
}

/// A current block after Alpenglow, in which 132 of 314 transactions are v1
/// messages. v0.5.1 decodes them without reading their version; native used to
/// reject the whole block on the first one. Every difference is a documented
/// v0.5.1 extraction loss: native keeps each v0.5.1 row of a transaction, in
/// order, and only adds rows.
#[test]
fn released_v051_current_block_with_v1_messages() {
    const PUMPSWAP: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
    const PUMPFUN: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
    const BUY: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
    const BUY_EXACT_QUOTE_IN: [u8; 8] = [198, 46, 21, 82, 180, 217, 232, 112];
    const BUY_EXACT_SOL_IN: [u8; 8] = [56, 252, 116, 8, 158, 223, 205, 95];
    let oracle = legacy_wasm::Oracle::new();
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../../../../spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let block = sol::Block::decode_from_slice(&bytes).unwrap();
    let clock = clock(&block);
    let old_events = oracle.events(&bytes).expect("historical block projector panicked");
    let oracle_rows = rows(DatabaseChanges::decode_from_slice(&oracle.format(&clock.encode_to_vec(), &old_events).unwrap()).unwrap());
    let native_rows = rows(changes(&clock, dex_swaps::project(&block).unwrap(), 100_000, 64 * 1024 * 1024).unwrap());
    // Row fields by transaction; ordinals shift after every added row.
    let by_signature = |rows: &Value| {
        let mut transactions: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for row in rows.as_array().unwrap() {
            let signature = row["fields"]["signature"].as_str().unwrap().to_string();
            transactions.entry(signature).or_default().push(row["fields"].clone());
        }
        transactions
    };
    let (oracle_transactions, native_transactions) = (by_signature(&oracle_rows), by_signature(&native_rows));
    let mut differences: BTreeMap<&str, usize> = BTreeMap::new();
    let mut v1_rows = [0, 0];
    for tx in &block.transactions {
        let signature = base58::encode(&tx.transaction.signatures[0]);
        let old = oracle_transactions.get(&signature).cloned().unwrap_or_default();
        let mut added = native_transactions.get(&signature).cloned().unwrap_or_default();
        if tx.transaction.message.version == Some(1) {
            v1_rows[0] += old.len();
            v1_rows[1] += added.len();
        }
        let mut cursor = 0;
        for row in &old {
            cursor += added[cursor..]
                .iter()
                .position(|candidate| candidate == row)
                .expect("native keeps every v0.5.1 row in order");
            added.remove(cursor);
        }
        *differences.entry("none").or_default() += old.len();
        let mut buys = [0, 0];
        for row in &added {
            let difference = match row["protocol"].as_str().unwrap() {
                // v0.5.1 decodes no DAMM v2 `EvtSwap2` (after 2025-12-26) and
                // no CLMM `SwapEvent` (after 2026-05-18); see the README.
                "meteora_daam" => "native_restores_damm_v2_leg",
                "raydium_clmm" => "native_restores_raydium_clmm_leg",
                "pumpfun_amm" => {
                    buys[0] += 1;
                    "native_restores_pumpswap_leg"
                }
                "pumpfun" => {
                    buys[1] += 1;
                    "native_restores_pumpfun_bonding_curve_leg"
                }
                other => panic!("unclassified native {other} row in {signature}"),
            };
            *differences.entry(difference).or_default() += 1;
        }
        if tx.meta.err.is_unset() {
            // Each restored PumpSwap or Pump.fun row is one current-format buy.
            let current = [
                current_buys(tx, PUMPSWAP, [BUY, BUY_EXACT_QUOTE_IN], None),
                current_buys(tx, PUMPFUN, [BUY, BUY_EXACT_SOL_IN], Some(BUY_EXACT_SOL_IN)),
            ];
            assert_eq!(buys, current, "{signature}");
        }
    }
    for protocol in ["meteora_daam", "raydium_clmm"] {
        assert!(oracle_rows.as_array().unwrap().iter().all(|row| row["fields"]["protocol"] != protocol));
    }
    assert_eq!(
        differences,
        BTreeMap::from([
            ("none", 29),
            ("native_restores_damm_v2_leg", 11),
            ("native_restores_pumpfun_bonding_curve_leg", 1),
            ("native_restores_pumpswap_leg", 19),
            ("native_restores_raydium_clmm_leg", 1),
        ])
    );
    assert_eq!((oracle_rows.as_array().unwrap().len(), native_rows.as_array().unwrap().len()), (29, 61));
    // Rows of v1 transactions, on both sides; native adds two DAMM v2 legs.
    assert_eq!(v1_rows, [8, 10]);
}
