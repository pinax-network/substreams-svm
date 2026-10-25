//! Native projection versus the Substreams module body, on the pinned blocks.
use buffa::Message;
use flate2::read::GzDecoder;
use spl_transfers::{project, Coverage, OperationKind, Projection, TokenProgram};
use spl_transfers_substreams::pb::solana::spl::transfers::v1 as pb;
use spl_transfers_substreams::{map_events_impl, view_block};
use std::io::Read;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

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

/// Compares field by field rather than through `to_proto`, so the mapping
/// itself is checked.
fn assert_same(native: &Projection, events: &pb::Events) {
    let clock = events.clock.as_option().expect("clock on every block");
    assert_eq!(clock.slot, native.clock.slot);
    assert_eq!(clock.parent_slot, native.clock.parent_slot);
    assert_eq!(clock.blockhash, native.clock.blockhash);
    assert_eq!(clock.previous_blockhash, native.clock.previous_blockhash);
    assert_eq!(clock.block_time, native.clock.block_time);
    assert_eq!(clock.block_height, native.clock.block_height);
    assert_eq!(clock.transaction_count, native.clock.transaction_count);
    assert_eq!(clock.spec_revision, native.clock.spec_revision);
    let coverage = clock.coverage.as_option().unwrap();
    assert_eq!(
        Coverage {
            token_2022: coverage.token_2022,
            inner_instructions: coverage.inner_instructions,
            unchecked_transfers: coverage.unchecked_transfers,
            mint_burn: coverage.mint_burn,
        },
        native.clock.coverage
    );

    assert_eq!(events.operations.len(), native.operations.len());
    for (record, operation) in events.operations.iter().zip(&native.operations) {
        assert_eq!(record.transaction_index, operation.transaction_index);
        assert_eq!(record.instruction_index, operation.instruction_index);
        assert_eq!(record.inner_instruction_index, operation.inner_instruction_index);
        assert_eq!(record.stack_height, operation.stack_height);
        assert_eq!(record.signature, operation.signature);
        let program = match record.program.to_i32() {
            1 => TokenProgram::Token,
            2 => TokenProgram::Token2022,
            _ => panic!("unknown or unspecified program"),
        };
        assert_eq!(program, operation.program);
        let kind = match record.kind.to_i32() {
            1 => OperationKind::Transfer,
            2 => OperationKind::TransferChecked,
            3 => OperationKind::MintTo,
            4 => OperationKind::MintToChecked,
            5 => OperationKind::Burn,
            6 => OperationKind::BurnChecked,
            _ => panic!("unknown or unspecified kind"),
        };
        assert_eq!(kind, operation.kind);
        assert_eq!(record.amount, operation.amount);
        assert_eq!(record.decimals, operation.decimals.map(u32::from));
        assert_eq!(record.mint.as_deref(), operation.mint.as_ref().map(|key| key.as_slice()));
        assert_eq!(record.source.as_deref(), operation.source.as_ref().map(|key| key.as_slice()));
        assert_eq!(record.destination.as_deref(), operation.destination.as_ref().map(|key| key.as_slice()));
        assert_eq!(record.authority, operation.authority);
        assert_eq!(record.extra_accounts.len(), operation.extra_accounts.len());
        for (left, right) in record.extra_accounts.iter().zip(&operation.extra_accounts) {
            assert_eq!(left, right);
        }
        assert_eq!(record.source_owner.as_deref(), operation.source_owner.as_ref().map(|key| key.as_slice()));
        assert_eq!(
            record.destination_owner.as_deref(),
            operation.destination_owner.as_ref().map(|key| key.as_slice())
        );
    }
}

fn assert_parity(block: &Block) {
    for params in ["", "all", "inner_instructions", "token_2022,mint_burn,unchecked_transfers"] {
        let native = project(&view_block(block).unwrap(), Coverage::parse(params).unwrap()).unwrap();
        let events = map_events_impl(params, block).unwrap();
        assert_same(&native, &events);
        // Serialized module output decodes back to the same records.
        assert_eq!(pb::Events::decode_from_slice(events.encode_to_vec().as_slice()).unwrap(), events);
    }
}

#[test]
fn real_blocks() {
    for fixture in FIXTURES {
        assert_parity(&decode_block(fixture));
    }
}

#[test]
fn missing_block_time_and_empty_block() {
    let mut block = decode_block(FIXTURES[1]);
    block.block_time = buffa::MessageField::none();
    assert_parity(&block);
    assert_eq!(map_events_impl("", &block).unwrap().clock.as_option().unwrap().block_time, None);

    block.transactions.clear();
    assert_parity(&block);
    let events = map_events_impl("all", &block).unwrap();
    assert!(events.operations.is_empty());
    assert_eq!(events.clock.as_option().unwrap().transaction_count, 0);
}

#[test]
fn malformed_input_fails_the_same_way() {
    let mut block = decode_block(FIXTURES[1]);
    let message = block.transactions[1366].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message.instructions[0].accounts[2] = 255;
    let native = project(&view_block(&block).unwrap(), Coverage::TRANSFER_CHECKED).unwrap_err();
    let wrapper = map_events_impl("", &block).unwrap_err();
    assert_eq!(wrapper.to_string(), native.to_string());
}

#[test]
fn unknown_coverage_flag_is_rejected() {
    let block = decode_block(FIXTURES[1]);
    assert!(map_events_impl("cpi", &block).is_err());
}

#[test]
fn unsupported_message_version_is_rejected_before_projection() {
    let mut block = decode_block(FIXTURES[1]);
    let message = block.transactions[1366].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
    message.versioned = true;
    message.version = Some(2);
    let error = map_events_impl("all", &block).unwrap_err();
    let error = error.downcast_ref::<spl_transfers::UnsupportedMessage>().expect("typed adapter error");
    assert_eq!(error.transaction_index, 1366);
    assert_eq!(error.version, Some(2));
}

#[test]
fn current_block_with_v1_messages_is_projected() {
    let block = decode_block(include_bytes!("../../transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz"));
    assert_parity(&block);
    let events = map_events_impl("all", &block).unwrap();
    let from_v1 = events
        .operations
        .iter()
        .filter(|record| block.transactions[record.transaction_index as usize].transaction.message.version == Some(1))
        .count();
    assert_eq!((events.operations.len(), from_v1), (296, 59));
}
