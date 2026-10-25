//! Latest Solana/Buffa and legacy Prost models feed the same borrowed projector.
mod common;

use buffa::{Message, MessageField};
use common::*;
use spl_transfers::{project, Coverage};
use substreams_solana_buffa::pb::sf::solana::r#type::v1 as buffa_pb;

spl_transfers::block_view_buffa!(fn view_buffa_block, buffa_pb);
spl_transfers::block_view_buffa_lenient!(fn view_buffa_block_lenient, buffa_pb);

fn buffa_block(fixture: &[u8]) -> buffa_pb::Block {
    buffa_pb::Block::decode_from_slice(&gunzip(fixture)).unwrap()
}

fn assert_equivalent(prost: &pb::Block, buffa: &buffa_pb::Block) {
    assert_eq!(view_block(prost), view_buffa_block(buffa).unwrap());
    for coverage in [Coverage::TRANSFER_CHECKED, Coverage::ALL] {
        assert_eq!(project(&view_block(prost), coverage), project(&view_buffa_block(buffa).unwrap(), coverage));
    }
}

#[test]
fn every_pinned_block_has_identical_borrowed_fields_and_projection() {
    for fixture in [BLOCK_349019252, BLOCK_350000009, BLOCK_350000010, BLOCK_350000026, BLOCK_461335700] {
        let prost = decode_block(fixture);
        let buffa = buffa_block(fixture);
        assert_equivalent(&prost, &buffa);

        let source = &buffa.transactions[0].transaction;
        let view = view_buffa_block(&buffa).unwrap();
        assert!(std::ptr::eq(view.transactions[0].signatures[0], source.signatures[0].as_slice()));
        assert!(std::ptr::eq(
            view.transactions[0].message.as_ref().unwrap().account_keys[0],
            source.message.account_keys[0].as_slice()
        ));
    }
}

#[test]
fn absent_required_messages_never_become_default_instances() {
    let prost = decode_block(BLOCK_350000009);
    let buffa = buffa_block(BLOCK_350000009);
    for field in ["transaction", "message", "header", "meta"] {
        let mut old = prost.clone();
        let mut new = buffa.clone();
        let old_tx = &mut old.transactions[1366];
        let new_tx = &mut new.transactions[1366];
        match field {
            "transaction" => {
                old_tx.transaction = None;
                new_tx.transaction = MessageField::none();
            }
            "message" => {
                old_tx.transaction.as_mut().unwrap().message = None;
                new_tx.transaction.as_option_mut().unwrap().message = MessageField::none();
            }
            "header" => {
                old_tx.transaction.as_mut().unwrap().message.as_mut().unwrap().header = None;
                new_tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().header = MessageField::none();
            }
            "meta" => {
                old_tx.meta = None;
                new_tx.meta = MessageField::none();
            }
            _ => unreachable!(),
        }
        assert_equivalent(&old, &new);
        assert!(project(&view_buffa_block(&new).unwrap(), Coverage::ALL).is_err(), "missing {field}");
    }
}

#[test]
fn present_default_error_still_marks_a_transaction_failed() {
    let mut old = decode_block(BLOCK_350000009);
    let mut new = buffa_block(BLOCK_350000009);
    old.transactions[1366].meta.as_mut().unwrap().err = Some(pb::TransactionError::default());
    new.transactions[1366].meta.as_option_mut().unwrap().err = MessageField::some(buffa_pb::TransactionError::default());
    assert_equivalent(&old, &new);
    let result = project(&view_buffa_block(&new).unwrap(), Coverage::ALL).unwrap();
    assert!(result.operations.iter().all(|operation| operation.transaction_index != 1366));
}

#[test]
fn optional_clock_fields_distinguish_absent_from_present_zero() {
    let mut old = decode_block(BLOCK_350000009);
    let mut new = buffa_block(BLOCK_350000009);
    old.block_time = None;
    new.block_time = MessageField::none();
    old.block_height = None;
    new.block_height = MessageField::none();
    assert_equivalent(&old, &new);
    assert_eq!(
        (view_buffa_block(&new).unwrap().block_time, view_buffa_block(&new).unwrap().block_height),
        (None, None)
    );

    old.block_time = Some(pb::UnixTimestamp::default());
    new.block_time = MessageField::some(buffa_pb::UnixTimestamp::default());
    old.block_height = Some(pb::BlockHeight::default());
    new.block_height = MessageField::some(buffa_pb::BlockHeight::default());
    assert_equivalent(&old, &new);
    assert_eq!(
        (view_buffa_block(&new).unwrap().block_time, view_buffa_block(&new).unwrap().block_height),
        (Some(0), Some(0))
    );
}

#[test]
fn only_legacy_v0_and_v1_message_shapes_are_admitted() {
    fn message(block: &mut buffa_pb::Block) -> &mut buffa_pb::Message {
        block.transactions[1366].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap()
    }
    let original = buffa_block(BLOCK_350000009);
    assert!(message(&mut original.clone()).address_table_lookups.is_empty());
    for (version, versioned, inline_configuration) in [
        (None, false, false),
        (None, true, false),
        (Some(0), true, false),
        (Some(1), true, false),
        (Some(1), true, true),
    ] {
        let mut block = original.clone();
        let message = message(&mut block);
        message.version = version;
        message.versioned = versioned;
        if inline_configuration {
            message.transaction_config = MessageField::some(buffa_pb::TransactionConfig::default());
        }
        assert!(view_buffa_block(&block).is_ok(), "{version:?} {versioned} {inline_configuration}");
    }
    for (version, versioned, inline_configuration, address_table_lookups) in [
        (Some(2), true, false, 0),
        (Some(2), true, true, 0),
        (Some(u32::MAX), true, false, 0),
        (Some(0), false, false, 0),
        (Some(1), false, false, 0),
        (Some(1), true, false, 1),
        (Some(1), true, true, 2),
        (None, false, true, 0),
        (Some(0), true, true, 0),
    ] {
        for failed in [false, true] {
            let mut block = original.clone();
            let message = message(&mut block);
            message.version = version;
            message.versioned = versioned;
            if inline_configuration {
                message.transaction_config = MessageField::some(buffa_pb::TransactionConfig::default());
            }
            message.address_table_lookups = vec![buffa_pb::MessageAddressTableLookup::default(); address_table_lookups];
            if failed {
                block.transactions[1366].meta.as_option_mut().unwrap().err = MessageField::some(buffa_pb::TransactionError::default());
            }
            assert_eq!(
                view_buffa_block(&block).unwrap_err(),
                spl_transfers::UnsupportedMessage {
                    transaction_index: 1366,
                    version,
                    versioned,
                    inline_configuration,
                    address_table_lookups,
                }
            );
        }
    }
}

#[test]
fn lenient_adapter_reports_unsupported_messages_without_dropping_positions() {
    let original = buffa_block(BLOCK_350000009);
    let (view, unsupported) = view_buffa_block_lenient(&original);
    assert!(unsupported.is_empty());
    assert_eq!(view, view_buffa_block(&original).unwrap());

    let mut block = original.clone();
    for index in [3, 1366] {
        let message = block.transactions[index].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.version = Some(2);
        message.versioned = true;
    }
    // Malformed, but only inside an unsupported transaction.
    block.transactions[1366]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .instructions[0]
        .accounts[2] = 255;

    let (view, unsupported) = view_buffa_block_lenient(&block);
    assert_eq!(view.transactions.len(), original.transactions.len());
    assert_eq!(unsupported.iter().map(|message| message.transaction_index).collect::<Vec<_>>(), [3, 1366]);
    assert_eq!(view_buffa_block(&block).unwrap_err(), unsupported[0]);
    assert!(spl_transfers::validate_block(&view).is_err());
    assert!(spl_transfers::validate_block_except(&view, &unsupported).is_ok());

    // Supported transactions are still validated.
    block.transactions[1365].transaction.as_option_mut().unwrap().signatures.clear();
    let (view, unsupported) = view_buffa_block_lenient(&block);
    assert!(spl_transfers::validate_block_except(&view, &unsupported).is_err());
}

#[test]
fn current_block_with_v1_messages_is_admitted_and_projected() {
    let bytes = gunzip(BLOCK_461335700);
    let block = buffa_pb::Block::decode_from_slice(&bytes).unwrap();
    // The repository's Buffa model reads every field: it re-encodes byte for byte.
    assert_eq!(block.encode_to_vec(), bytes);
    let message = |index: usize| block.transactions[index].transaction.message.as_option().unwrap();
    let v1: Vec<_> = (0..block.transactions.len()).filter(|&index| message(index).version == Some(1)).collect();
    let v0 = (0..block.transactions.len()).filter(|&index| message(index).version == Some(0)).count();
    assert_eq!((block.transactions.len(), v0, v1.len()), (314, 100, 132));
    for &index in &v1 {
        let message = message(index);
        assert!(message.versioned && message.address_table_lookups.is_empty() && message.transaction_config.is_set());
    }

    let view = view_buffa_block(&block).unwrap();
    assert!(view_buffa_block_lenient(&block).1.is_empty());
    let projection = project(&view, Coverage::ALL).unwrap();
    let from_v1 = projection
        .operations
        .iter()
        .filter(|operation| v1.contains(&(operation.transaction_index as usize)))
        .count();
    assert_eq!((projection.operations.len(), from_v1), (296, 59));

    // Newer versions and a v1 message with address table lookups still fail closed.
    let mut newer = block.clone();
    newer.transactions[v1[0]]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .version = Some(2);
    let error = view_buffa_block(&newer).unwrap_err();
    assert_eq!((error.transaction_index, error.version), (v1[0], Some(2)));
    let mut lookups = block.clone();
    lookups.transactions[v1[1]]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .address_table_lookups
        .push(buffa_pb::MessageAddressTableLookup::default());
    let error = view_buffa_block(&lookups).unwrap_err();
    assert_eq!((error.transaction_index, error.version, error.address_table_lookups), (v1[1], Some(1), 1));
    let (_, unsupported) = view_buffa_block_lenient(&lookups);
    assert_eq!(unsupported, [error]);
}
