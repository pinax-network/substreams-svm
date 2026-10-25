//! The `spl_token` module body against the native `spl_transfers` projector.
use buffa::{Message, MessageField};
use flate2::read::GzDecoder;
use proto::pb::solana::spl::token::v1 as pb;
use spl_token_common::{process_block, SOLANA_TOKEN_PROGRAM_KEG, SOLANA_TOKEN_PROGRAM_ZQB};
use spl_transfers::{project, Coverage, OperationKind};
use std::io::Read;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

spl_transfers::block_view_buffa!(fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

const FIXTURES: [&[u8]; 4] = [
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-349019252.pb.gz"),
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-350000009.pb.gz"),
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-350000010.pb.gz"),
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-350000026.pb.gz"),
];

fn decode_block(fixture: &[u8]) -> Block {
    let mut bytes = Vec::new();
    GzDecoder::new(fixture).read_to_end(&mut bytes).unwrap();
    Block::decode_from_slice(bytes.as_slice()).unwrap()
}

fn is_spl_token_program(program_id: &[u8]) -> bool {
    program_id == SOLANA_TOKEN_PROGRAM_KEG || program_id == SOLANA_TOKEN_PROGRAM_ZQB
}

#[derive(Debug, PartialEq)]
struct Row {
    position: (u32, u32, Option<u32>),
    class: &'static str,
    amount: u64,
    decimals: Option<u32>,
    mint: Option<Vec<u8>>,
    source: Vec<u8>,
    destination: Vec<u8>,
    authority: Vec<u8>,
    extra_accounts: Vec<Vec<u8>>,
}

fn module_rows(events: &pb::Events) -> Vec<Row> {
    let mut rows = Vec::new();
    for transaction in &events.transactions {
        for instruction in &transaction.instructions {
            let (class, data) = match &instruction.instruction {
                Some(pb::instruction::Instruction::Transfer(data)) => ("transfer", data),
                Some(pb::instruction::Instruction::Mint(data)) => ("mint", data),
                Some(pb::instruction::Instruction::Burn(data)) => ("burn", data),
                _ => continue,
            };
            rows.push(Row {
                position: (
                    transaction.transaction_index,
                    instruction.instruction_index,
                    instruction.inner_instruction_index,
                ),
                class,
                amount: data.amount,
                decimals: data.decimals,
                mint: data.mint.clone(),
                source: data.source.clone(),
                destination: data.destination.clone(),
                authority: data.authority.clone(),
                extra_accounts: data.multisig_authority.clone(),
            });
        }
    }
    rows
}

fn projector_rows(block: &Block) -> Vec<Row> {
    let projection = project(&view_block(block).unwrap(), Coverage::ALL).unwrap();
    projection
        .operations
        .iter()
        .map(|operation| Row {
            position: (operation.transaction_index, operation.instruction_index, operation.inner_instruction_index),
            class: match operation.kind {
                OperationKind::Transfer | OperationKind::TransferChecked => "transfer",
                OperationKind::MintTo | OperationKind::MintToChecked => "mint",
                OperationKind::Burn | OperationKind::BurnChecked => "burn",
            },
            amount: operation.amount,
            decimals: operation.decimals.map(u32::from),
            mint: operation.mint.map(|key| key.to_vec()),
            source: operation.source.map(|key| key.to_vec()).unwrap_or_default(),
            destination: operation.destination.map(|key| key.to_vec()).unwrap_or_default(),
            authority: operation.authority.to_vec(),
            extra_accounts: operation.extra_accounts.iter().map(|key| key.to_vec()).collect(),
        })
        .collect()
}

#[test]
fn transfer_records_match_the_native_projector() {
    for fixture in FIXTURES {
        let block = decode_block(fixture);
        let events = process_block(&block, is_spl_token_program).unwrap();
        let rows = module_rows(&events);
        assert!(!rows.is_empty());
        assert_eq!(rows, projector_rows(&block));
    }
}

#[test]
fn positions_are_original() {
    let block = decode_block(FIXTURES[3]);
    let events = process_block(&block, is_spl_token_program).unwrap();
    for transaction in &events.transactions {
        let original = &block.transactions[transaction.transaction_index as usize];
        assert_eq!(transaction.signature, original.transaction.as_option().unwrap().signatures[0]);
        assert!(!original.meta.as_option().unwrap().err.is_set());
        let message = original.transaction.as_option().unwrap().message.as_option().unwrap();
        for instruction in &transaction.instructions {
            let program_index = match instruction.inner_instruction_index {
                None => message.instructions[instruction.instruction_index as usize].program_id_index,
                Some(inner) => {
                    let meta = original.meta.as_option().unwrap();
                    let group = meta
                        .inner_instructions
                        .iter()
                        .find(|group| group.index == instruction.instruction_index)
                        .unwrap();
                    group.instructions[inner as usize].program_id_index
                }
            };
            assert_eq!(instruction.is_root, instruction.inner_instruction_index.is_none());
            if (program_index as usize) < message.account_keys.len() {
                assert_eq!(message.account_keys[program_index as usize], instruction.program_id);
            }
        }
    }
}

#[test]
fn zero_self_and_unchecked_transfers() {
    let block = decode_block(FIXTURES[3]);
    let rows = module_rows(&process_block(&block, is_spl_token_program).unwrap());
    assert!(rows.iter().any(|row| row.class == "transfer" && row.source == row.destination));
    let unchecked: Vec<_> = rows.iter().filter(|row| row.class == "transfer" && row.decimals.is_none()).collect();
    assert!(!unchecked.is_empty());
    assert!(unchecked.iter().all(|row| row.mint.is_none() && row.destination != row.authority));
    assert!(rows.iter().filter(|row| row.class == "mint").all(|row| row.source.is_empty()));

    let block = decode_block(FIXTURES[1]);
    let rows = module_rows(&process_block(&block, is_spl_token_program).unwrap());
    assert!(rows.iter().any(|row| row.position == (1366, 0, None) && row.amount == 0));
}

#[test]
fn malformed_input_is_an_error_not_a_panic() {
    let mut block = decode_block(FIXTURES[1]);
    let message = block.transactions[1366].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message.instructions[0].accounts[2] = 255;
    assert!(process_block(&block, is_spl_token_program).is_err());

    let mut block = decode_block(FIXTURES[1]);
    let message = block.transactions[1366].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message.instructions[0].accounts.truncate(2);
    let error = process_block(&block, is_spl_token_program).unwrap_err().to_string();
    assert!(error.contains("instruction 0") && error.contains("transfer_checked"), "{error}");
}

/// Marks transaction `index` as an explicit message of `version`.
fn mark_version(block: &mut Block, index: usize, version: u32) {
    let message = block.transactions[index].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message.versioned = true;
    message.version = Some(version);
}

/// Marks transaction `index` as an explicit v2 message, which the shared
/// decoder does not cover.
fn mark_v2(block: &mut Block, index: usize) {
    mark_version(block, index, 2);
}

#[test]
fn v1_messages_are_processed_not_skipped() {
    let block = decode_block(FIXTURES[1]);
    let expected = process_block(&block, is_spl_token_program).unwrap();
    let mut v1 = block.clone();
    mark_version(&mut v1, 1366, 1);
    for configured in [false, true] {
        if configured {
            v1.transactions[1366]
                .transaction
                .as_option_mut()
                .unwrap()
                .message
                .as_option_mut()
                .unwrap()
                .transaction_config = MessageField::some(Default::default());
        }
        assert_eq!(process_block(&v1, is_spl_token_program).unwrap(), expected);
    }

    // A current block: about 40% of its transactions are v1 messages.
    let block = decode_block(include_bytes!("../../transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz"));
    let events = process_block(&block, is_spl_token_program).unwrap();
    let rows = module_rows(&events);
    assert_eq!(rows, projector_rows(&block));
    let is_v1 = |index: u32| block.transactions[index as usize].transaction.message.version == Some(1);
    let v1_transactions = events.transactions.iter().filter(|transaction| is_v1(transaction.transaction_index)).count();
    let v1_rows = rows.iter().filter(|row| is_v1(row.position.0)).count();
    assert_eq!((events.transactions.len(), v1_transactions, rows.len(), v1_rows), (109, 22, 296, 59));
}

#[test]
fn unsupported_messages_are_skipped_without_halting() {
    let block = decode_block(FIXTURES[1]);
    let expected = process_block(&block, is_spl_token_program).unwrap();
    assert!(expected.transactions.iter().any(|transaction| transaction.transaction_index == 1366));

    let mut v2 = block.clone();
    mark_v2(&mut v2, 1366);
    let mut configured = block.clone();
    configured.transactions[1366]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .transaction_config = MessageField::some(Default::default());

    for block in [v2, configured] {
        let events = process_block(&block, is_spl_token_program).unwrap();
        // Only the unsupported transaction is missing; every other transaction
        // keeps its original index and content.
        let remaining: Vec<_> = expected
            .transactions
            .iter()
            .filter(|transaction| transaction.transaction_index != 1366)
            .cloned()
            .collect();
        assert_eq!(events.transactions, remaining);
    }
}

#[test]
fn unsupported_messages_are_not_validated_but_others_still_are() {
    // A malformed unsupported transaction is skipped rather than rejected.
    let mut block = decode_block(FIXTURES[1]);
    mark_v2(&mut block, 1366);
    block.transactions[1366]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .instructions[0]
        .accounts[2] = 255;
    assert!(process_block(&block, is_spl_token_program).is_ok());

    // Malformed supported transactions still fail the block.
    let mut block = decode_block(FIXTURES[1]);
    mark_v2(&mut block, 0);
    block.transactions[1366]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .instructions[0]
        .accounts[2] = 255;
    assert!(process_block(&block, is_spl_token_program).is_err());
}
