//! Execute the published package, including its vote filter, without a provider.
use super::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[path = "../../../tests/support/legacy_wasm.rs"]
mod legacy_wasm;

fn cases() -> Vec<(&'static str, sol::Block)> {
    let mut cases = vec![
        ("empty", block(vec![])),
        ("system_transfer", block(vec![transaction(10, vec![transfer(42, 1)])])),
        ("system_max", block(vec![transaction(10, vec![transfer(u64::MAX, 1)])])),
        ("system_zero", block(vec![transaction(10, vec![transfer(0, 1)])])),
        ("system_self", block(vec![transaction(10, vec![transfer(42, 0)])])),
    ];
    for (name, tag, amount, program) in [
        ("spl_checked", 12, 42, 5),
        ("spl_max", 12, u64::MAX, 5),
        ("spl_zero", 12, 0, 5),
        ("spl_unchecked", 3, 42, 5),
        ("mint_unchecked", 7, 42, 5),
        ("mint_checked", 14, 42, 5),
        ("burn_unchecked", 8, 42, 5),
        ("burn_checked", 15, 42, 5),
        ("token2022_checked", 12, 42, 6),
    ] {
        cases.push((name, block(vec![transaction(10, vec![token(tag, amount, program)])])));
    }
    let mut tx = transaction(10, vec![token(12, 42, 5)]);
    tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].accounts[2] = 0;
    cases.push(("spl_self", block(vec![tx])));
    let mut tx = transaction(10, vec![token(12, 42, 5)]);
    tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0]
        .accounts
        .extend([0, 1]);
    cases.push(("multisig_extra_accounts", block(vec![tx])));
    let mut tx = transaction(10, vec![token(12, 42, 5), transfer(43, 1)]);
    tx.meta.as_option_mut().unwrap().err = sol::TransactionError::default().into();
    cases.push(("failed_transaction", block(vec![tx])));
    let mut tx = transaction(10, vec![token(12, 42, 5)]);
    let inner = token(3, 43, 6);
    tx.meta.as_option_mut().unwrap().inner_instructions.push(sol::InnerInstructions {
        index: 0,
        instructions: vec![sol::InnerInstruction {
            program_id_index: inner.program_id_index,
            accounts: inner.accounts,
            data: inner.data,
            stack_height: Some(2),
        }],
    });
    cases.push(("spl_cpi", block(vec![tx])));
    let mut tx = transaction(10, vec![transfer(42, 1)]);
    let inner = transfer(43, 1);
    tx.meta.as_option_mut().unwrap().inner_instructions.push(sol::InnerInstructions {
        index: 0,
        instructions: vec![sol::InnerInstruction {
            program_id_index: inner.program_id_index,
            accounts: inner.accounts,
            data: inner.data,
            stack_height: Some(2),
        }],
    });
    cases.push(("system_cpi", block(vec![tx])));
    let mut tx = transaction(10, vec![transfer(42, 1)]);
    tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data = vec![255];
    cases.push(("malformed_system", block(vec![tx])));
    let mut tx = transaction(10, vec![token(12, 42, 5)]);
    tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().version = Some(1);
    cases.push(("unsupported_message_version", block(vec![tx])));
    let mut vote = transaction(11, vec![]);
    let message = vote.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message.account_keys[4] = base58::decode("Vote111111111111111111111111111111111111111").unwrap();
    message.instructions.push(sol::CompiledInstruction {
        program_id_index: 4,
        accounts: vec![],
        data: vec![],
    });
    cases.push((
        "vote_then_transfers",
        block(vec![vote, transaction(12, vec![token(12, 42, 5), transfer(43, 1)])]),
    ));
    cases.push((
        "balance_only_then_transfers",
        block(vec![transaction(11, vec![]), transaction(12, vec![token(12, 42, 5), transfer(43, 1)])]),
    ));
    cases
}

fn rows(changes: DatabaseChanges) -> Vec<BTreeMap<String, String>> {
    changes
        .table_changes
        .into_iter()
        .filter(|row| matches!(row.table.as_str(), "spl_transfer" | "system_transfer"))
        .map(|row| {
            let mut fields: BTreeMap<_, _> = row.fields.into_iter().map(|field| (field.name, field.value)).collect();
            fields.insert("@table".into(), row.table);
            fields
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn published_v030_boundary_corpus() {
    let oracle = legacy_wasm::Oracle::new();
    let mut corpus = Vec::new();
    for (name, block) in cases() {
        let bytes = block.encode_to_vec();
        let clock = clock(&block);
        let oracle_bytes = oracle.database(&clock.encode_to_vec(), &bytes);
        let legacy_rows = oracle_bytes.as_ref().map(|bytes| rows(DatabaseChanges::decode_from_slice(bytes).unwrap()));
        let native = view(&block).map_err(|_| "unsupported_message").and_then(|view| {
            let spl = spl_transfers::project(&view, Coverage::ALL).map_err(|_| "invalid_spl")?;
            let system = native_token::project(&block).map_err(|_| "invalid_system")?;
            changes(&clock, &spl, &system, 100_000, 4 * 1024 * 1024).map_err(|_| "invalid_format")
        });
        let native_bytes = native.as_ref().ok().map(|changes| {
            let mut canonical = changes.clone();
            for row in &mut canonical.table_changes {
                // PK map/field iteration is not wire-contract ordering. Drop
                // hidden primary keys and sort fields for a stable reference.
                row.primary_key = None;
                row.fields.sort_by(|left, right| left.name.cmp(&right.name));
            }
            canonical.encode_to_vec()
        });
        let native = native.map(rows);
        let equal = native.as_ref().ok() == legacy_rows.as_ref();
        let difference = match name {
            "spl_zero" => "native_keeps_zero_amount",
            "spl_self" => "native_keeps_self_transfer",
            "spl_unchecked" | "spl_cpi" => "native_unchecked_destination_and_unknown_mint",
            "malformed_system" => "native_rejects_malformed_system_instruction",
            "unsupported_message_version" => "native_rejects_unsupported_message_version",
            _ => "none",
        };
        assert_eq!(equal, difference == "none", "unclassified package difference: {name}");
        match name {
            "spl_zero" | "spl_self" => {
                assert!(legacy_rows.as_ref().unwrap().is_empty());
                assert_eq!(native.as_ref().unwrap().len(), 1);
            }
            "spl_unchecked" | "spl_cpi" => {
                let old = legacy_rows.as_ref().unwrap().last().unwrap();
                let current = native.as_ref().unwrap().last().unwrap();
                assert_eq!(old["mint"], base58::encode([2; 32]));
                assert_eq!(old["destination"], base58::encode([4; 32]));
                assert_eq!(current["destination"], base58::encode([2; 32]));
                assert_eq!(current["mint"], "");
            }
            "malformed_system" => assert_eq!(native.as_ref().unwrap_err(), &"invalid_system"),
            "unsupported_message_version" => assert_eq!(native.as_ref().unwrap_err(), &"unsupported_message"),
            _ => {}
        }
        println!(
            "{name}: equal={equal}, legacy={}, native={}",
            legacy_rows.as_ref().map_or("panic".into(), |v| v.len().to_string()),
            native.as_ref().map_or_else(|e| e.to_string(), |v| v.len().to_string())
        );
        corpus.push(json!({
            "name":name, "block_hex":hex(&bytes), "block_sha256":format!("{:x}",Sha256::digest(&bytes)),
            "oracle_database_changes_hex":oracle_bytes.as_ref().map(|bytes| hex(bytes)),
            "native_database_changes_hex":native_bytes.as_ref().map(|bytes| hex(bytes)),
            "oracle_rows":legacy_rows, "native_rows":native.as_ref().ok(), "native_error":native.as_ref().err(),
            "equal_database_row_fields":equal, "difference":difference,
        }));
    }
    let value = json!({"schema_version":1,"package_sha256":"bc04960b7b18223e124a3ad5c3384817df0f66003a1ea4b989b9a80047e5b05e", "cases":corpus});
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy-v0.3.0/corpus.json");
    if std::env::var_os("UPDATE_SVM_LEGACY_CORPUS").as_deref() == Some(std::ffi::OsStr::new("1")) {
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
    }
    let expected: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(value, expected);
}
