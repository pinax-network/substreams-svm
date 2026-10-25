use super::*;
use std::{collections::BTreeMap, io::Read};
use substreams::buffa::MessageField;
use substreams_database_change::pb::sf::substreams::sink::database::v1::table_change::PrimaryKey;
use substreams_solana::pb::sf::solana::r#type::v1 as sol;

mod committed;
mod legacy_v030;

spl_transfers::block_view_buffa!(fn view, substreams_solana::pb::sf::solana::r#type::v1);

fn clock(block: &sol::Block) -> Clock {
    let mut clock = Clock {
        number: block.slot,
        id: block.blockhash.clone(),
        ..Default::default()
    };
    clock.timestamp = Default::default();
    clock.timestamp.get_or_insert_default().seconds = block.block_time.timestamp;
    clock
}

fn canonical(mut changes: DatabaseChanges) -> DatabaseChanges {
    // Field order is a HashMap implementation detail; row ordinals are not.
    for row in &mut changes.table_changes {
        row.fields.sort_by(|left, right| left.name.cmp(&right.name));
    }
    changes
}

fn transaction(signature: u8, instructions: Vec<sol::CompiledInstruction>) -> sol::ConfirmedTransaction {
    sol::ConfirmedTransaction {
        transaction: sol::Transaction {
            signatures: vec![vec![signature; 64]],
            message: sol::Message {
                header: sol::MessageHeader {
                    num_required_signatures: 1,
                    ..Default::default()
                }
                .into(),
                account_keys: vec![
                    vec![1; 32],
                    vec![2; 32],
                    vec![3; 32],
                    vec![4; 32],
                    vec![0; 32],
                    spl_transfers::TOKEN_PROGRAM_ID.to_vec(),
                    spl_transfers::TOKEN_2022_PROGRAM_ID.to_vec(),
                ],
                instructions,
                ..Default::default()
            }
            .into(),
        }
        .into(),
        meta: sol::TransactionStatusMeta {
            fee: 5000,
            pre_balances: vec![100; 7],
            post_balances: vec![100; 7],
            ..Default::default()
        }
        .into(),
    }
}

fn block(transactions: Vec<sol::ConfirmedTransaction>) -> sol::Block {
    sol::Block {
        slot: 100,
        parent_slot: 98,
        blockhash: base58::encode([7; 32]),
        previous_blockhash: base58::encode([8; 32]),
        block_time: sol::UnixTimestamp { timestamp: 1_700_000_000 }.into(),
        transactions,
        ..Default::default()
    }
}

fn transfer(lamports: u64, destination: u8) -> sol::CompiledInstruction {
    let mut data = 2u32.to_le_bytes().to_vec();
    data.extend(lamports.to_le_bytes());
    sol::CompiledInstruction {
        program_id_index: 4,
        accounts: vec![0, destination],
        data,
    }
}

/// `SystemInstruction::CreateAccount`, which the formatter writes no row for.
fn create_account(lamports: u64, new_account: u8) -> sol::CompiledInstruction {
    let mut data = 0u32.to_le_bytes().to_vec();
    data.extend(lamports.to_le_bytes());
    data.extend(100u64.to_le_bytes());
    data.extend([9; 32]);
    sol::CompiledInstruction {
        program_id_index: 4,
        accounts: vec![0, new_account],
        data,
    }
}

fn token(tag: u8, amount: u64, program: u32) -> sol::CompiledInstruction {
    let checked = [12, 14, 15].contains(&tag);
    let mut data = vec![tag];
    data.extend(amount.to_le_bytes());
    if checked {
        data.push(9);
    }
    let accounts = match tag {
        12 => vec![0, 2, 1, 3],
        3 => vec![0, 1, 3],
        7 | 14 => vec![2, 1, 3],
        8 | 15 => vec![0, 2, 3],
        _ => unreachable!(),
    };
    sol::CompiledInstruction {
        program_id_index: program,
        accounts,
        data,
    }
}

fn formatted(block: &sol::Block) -> DatabaseChanges {
    let spl = spl_transfers::project(&view(block).unwrap(), Coverage::ALL).unwrap();
    let system = native_token::project(block).unwrap();
    changes(&clock(block), &spl, &system, 100_000, 64 * 1024 * 1024).unwrap()
}

#[test]
fn full_spl_rows_match_wrapper_with_zero_self_mint_burn_and_cpi() {
    let mut instructions: Vec<_> = [3, 12, 7, 14, 8, 15]
        .into_iter()
        .enumerate()
        .map(|(i, tag)| token(tag, if i == 0 { 0 } else { u64::MAX }, if i % 2 == 0 { 5 } else { 6 }))
        .collect();
    instructions[0].accounts[1] = 0; // zero self transfer remains a row
    let mut tx = transaction(10, instructions);
    let inner = token(12, 42, 6);
    tx.meta.as_option_mut().unwrap().inner_instructions.push(sol::InnerInstructions {
        index: 1,
        instructions: vec![sol::InnerInstruction {
            program_id_index: inner.program_id_index,
            accounts: inner.accounts,
            data: inner.data,
            stack_height: Some(2),
        }],
    });
    let mut failed = tx.clone();
    failed.meta.as_option_mut().unwrap().err = sol::TransactionError::default().into();
    let block = block(vec![failed, tx]);
    let raw = spl_token_common::process_block(&block, |id| id == spl_transfers::TOKEN_PROGRAM_ID || id == spl_transfers::TOKEN_2022_PROGRAM_ID).unwrap();
    let expected = crate::changes(&clock(&block), &raw, &system::Events::default()).unwrap();
    let actual = formatted(&block);
    assert_eq!(actual.table_changes.iter().filter(|row| row.table == "spl_transfer").count(), 7);
    assert_eq!(canonical(actual), canonical(expected));
}

#[test]
fn system_rows_preserve_dense_indexes_and_nonrow_operations() {
    let mut failed = transaction(11, vec![transfer(1, 1)]);
    failed.meta.as_option_mut().unwrap().err = sol::TransactionError::default().into();
    let block = block(vec![
        transaction(10, vec![]), // balance-only: reserves index zero
        failed,                  // omitted entirely
        transaction(12, vec![create_account(42, 1), transfer(42, 0), transfer(0, 1), transfer(u64::MAX, 1)]),
    ]);
    let changes = formatted(&block);
    assert_eq!(changes.table_changes.len(), 2); // one transfer, one block
    let row = &changes.table_changes[0];
    assert_eq!(row.table, "system_transfer");
    let Some(PrimaryKey::CompositePk(key)) = &row.primary_key else {
        panic!("composite key")
    };
    assert_eq!(key.keys["transaction_index"], "1");
    assert_eq!(key.keys["instruction_index"], "2"); // create+self occupy slots; zero does not
    let fields: BTreeMap<_, _> = row.fields.iter().map(|field| (field.name.as_str(), field.value.as_str())).collect();
    assert_eq!(fields["lamports"], u64::MAX.to_string());
    assert_eq!(fields["fee"], "5000");
    assert_eq!(fields["compute_units_consumed"], "0");
    assert_eq!(fields["signature"], base58::encode([12; 64]));
    assert_eq!(fields["source"], base58::encode([1; 32]));
    assert_eq!(fields["destination"], base58::encode([2; 32]));
    assert_eq!(row.ordinal, 0);
    assert_eq!(changes.table_changes[1].table, "blocks");
}

#[test]
fn mainnet_spl_rows_match_current_wrapper_and_system_rows_are_present() {
    // A 2025 provider block, then a current block in which 132 of 314
    // transactions are v1 messages.
    for fixture in [
        include_bytes!("../../../spl/transfers/tests/fixtures/solana-mainnet-350000026.pb.gz").as_slice(),
        include_bytes!("../../../spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz").as_slice(),
    ] {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fixture).read_to_end(&mut bytes).unwrap();
        let block = sol::Block::decode_from_slice(&bytes).unwrap();
        let raw = spl_token_common::process_block(&block, |id| id == spl_transfers::TOKEN_PROGRAM_ID || id == spl_transfers::TOKEN_2022_PROGRAM_ID).unwrap();
        let mut expected = crate::changes(&clock(&block), &raw, &system::Events::default()).unwrap();
        expected.table_changes.retain(|row| row.table == "spl_transfer");
        let mut actual = formatted(&block);
        assert!(actual.table_changes.iter().any(|row| row.table == "system_transfer"));
        actual.table_changes.retain(|row| row.table == "spl_transfer");
        assert!(!actual.table_changes.is_empty());
        assert_eq!(canonical(actual), canonical(expected), "{}", block.slot);
    }
}

#[test]
fn loaded_system_accounts_and_cpi_keep_dense_order_and_metadata() {
    let mut tx = transaction(12, vec![transfer(43, 1)]);
    let msg = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    msg.version = Some(0);
    msg.versioned = true;
    let destination = msg.account_keys.pop().unwrap();
    msg.instructions[0].accounts[1] = 6;
    msg.address_table_lookups.push(sol::MessageAddressTableLookup {
        account_key: vec![9; 32],
        writable_indexes: vec![7],
        readonly_indexes: vec![],
    });
    let instruction = msg.instructions[0].clone();
    let meta = tx.meta.as_option_mut().unwrap();
    meta.loaded_writable_addresses.push(destination.clone());
    meta.compute_units_consumed = Some(12345);
    meta.inner_instructions.push(sol::InnerInstructions {
        index: 0,
        instructions: vec![sol::InnerInstruction {
            program_id_index: instruction.program_id_index,
            accounts: instruction.accounts,
            data: instruction.data,
            stack_height: Some(2),
        }],
    });
    let changes = formatted(&block(vec![tx]));
    assert_eq!(changes.table_changes.len(), 3);
    for (index, row) in changes.table_changes[..2].iter().enumerate() {
        let Some(PrimaryKey::CompositePk(key)) = &row.primary_key else {
            panic!("composite key")
        };
        assert_eq!(key.keys["instruction_index"], index.to_string());
        let fields: BTreeMap<_, _> = row.fields.iter().map(|field| (field.name.as_str(), field.value.as_str())).collect();
        assert_eq!(fields["destination"], base58::encode(&destination));
        assert_eq!(fields["compute_units_consumed"], "12345");
        assert_eq!(fields["stack_height"], if index == 0 { "0" } else { "2" });
    }
}

#[test]
fn identity_coverage_and_capacity_mismatch_fail_closed_and_empty_blocks_stay_empty() {
    let block = block(vec![transaction(10, vec![token(12, 42, 5), transfer(43, 1)])]);
    let mut spl = spl_transfers::project(&view(&block).unwrap(), Coverage::ALL).unwrap();
    let system = native_token::project(&block).unwrap();
    let clock = clock(&block);
    assert!(changes(&clock, &spl, &system, 1, 64 * 1024 * 1024).is_err());
    assert!(changes(&clock, &spl, &system, 100, 1).is_err());
    let mut wrong_clock = clock.clone();
    wrong_clock.number += 1;
    assert!(changes(&wrong_clock, &spl, &system, 100, 64 * 1024 * 1024).is_err());
    wrong_clock = clock.clone();
    wrong_clock.timestamp = MessageField::none();
    assert!(changes(&wrong_clock, &spl, &system, 100, 64 * 1024 * 1024).is_err());
    assert!(crate::changes(&wrong_clock, &spl::Events::default(), &system::Events::default()).is_err());
    spl.clock.coverage = Coverage::TRANSFER_CHECKED;
    assert!(changes(&clock, &spl, &system, 100, 64 * 1024 * 1024).is_err());
    assert!(formatted(&super::tests::block(vec![])).table_changes.is_empty());
}

#[test]
fn vote_key_transactions_take_a_system_slot_only_with_system_rows() {
    assert_eq!(base58::encode(VOTE_PROGRAM), "Vote111111111111111111111111111111111111111");
    let vote_key = |mut tx: sol::ConfirmedTransaction| {
        tx.transaction
            .as_option_mut()
            .unwrap()
            .message
            .as_option_mut()
            .unwrap()
            .account_keys
            .push(VOTE_PROGRAM.to_vec());
        let meta = tx.meta.as_option_mut().unwrap();
        meta.pre_balances.push(100);
        meta.post_balances.push(100);
        tx
    };
    let block = block(vec![
        vote_key(transaction(10, vec![])),                                       // dropped by the legacy input: no slot
        transaction(11, vec![transfer(41, 1)]),                                  // slot 0, as in the legacy package
        vote_key(transaction(12, vec![create_account(42, 3)])),                  // no row, so no slot either
        vote_key(transaction(13, vec![transfer(43, 0)])),                        // a self transfer writes no row: no slot
        transaction(14, vec![transfer(44, 1)]),                                  // slot 1, as in the legacy package
        vote_key(transaction(15, vec![create_account(45, 3), transfer(46, 1)])), // kept natively: slot 2
        transaction(16, vec![transfer(47, 1)]),                                  // slot 3; the legacy package uses 2
    ]);
    let slots: Vec<_> = formatted(&block)
        .table_changes
        .iter()
        .filter(|row| row.table == "system_transfer")
        .map(|row| {
            let Some(PrimaryKey::CompositePk(key)) = &row.primary_key else {
                panic!("composite key")
            };
            let lamports = row.fields.iter().find(|field| field.name == "lamports").unwrap();
            format!("{} {}.{}", lamports.value, key.keys["transaction_index"], key.keys["instruction_index"])
        })
        .collect();
    assert_eq!(slots, ["41 0.0", "44 1.0", "46 2.1", "47 3.0"]);
}

#[test]
fn system_row_predicate_matches_the_formatter() {
    use system::instruction::Instruction as I;
    let transfer = |destination: u8| system::Transfer {
        source: vec![1; 32],
        destination: vec![destination; 32],
        lamports: 42,
    };
    for (instruction, row) in [
        (I::Transfer(transfer(2).into()), true),
        (I::Transfer(transfer(1).into()), false),
        (I::TransferWithSeed(Default::default()), true),
        (I::CreateAccount(Default::default()), false),
        (I::CreateAccountWithSeed(Default::default()), false),
        (I::WithdrawNonceAccount(Default::default()), true),
    ] {
        let instruction = system::Instruction {
            instruction: Some(instruction),
            ..Default::default()
        };
        let events = system::Events {
            transactions: vec![system::Transaction {
                signature: vec![10; 64],
                instructions: vec![instruction.clone()],
                ..Default::default()
            }],
        };
        let changes = crate::changes(&clock(&block(vec![])), &spl::Events::default(), &events).unwrap();
        let rows = changes.table_changes.iter().filter(|change| change.table != "blocks").count();
        assert_eq!((emits_system_row(&instruction), rows), (row, usize::from(row)), "{instruction:?}");
    }
}

#[test]
fn hand_built_projection_without_a_signature_is_an_error() {
    let block = block(vec![transaction(10, vec![])]);
    let spl = spl_transfers::project(&view(&block).unwrap(), Coverage::ALL).unwrap();
    let mut unsigned = block.clone();
    unsigned.transactions[0].transaction.as_option_mut().unwrap().signatures.clear();
    let system = native_token::Projection {
        block: &unsigned,
        operations: Vec::new(),
    };
    let error = changes(&clock(&block), &spl, &system, 100, 64 * 1024 * 1024).unwrap_err();
    assert_eq!(error.to_string(), "missing transaction signature");
}
