mod common;

use common::*;
use spl_transfers::{project, validate_block, Coverage, DecodeError, OperationKind, Position, ProjectError, Projection, TokenProgram, TransactionError};

fn projection(block: &pb::Block, coverage: Coverage) -> Projection {
    project(&view_block(block), coverage).expect("valid block")
}

fn transaction_error(block: &pb::Block) -> (u32, TransactionError) {
    match project(&view_block(block), Coverage::ALL) {
        Err(ProjectError::Transaction { transaction_index, error }) => (transaction_index, error),
        other => panic!("expected a transaction error, got {other:?}"),
    }
}

fn message_mut(block: &mut pb::Block, transaction_index: usize) -> &mut pb::Message {
    block.transactions[transaction_index].transaction.as_mut().unwrap().message.as_mut().unwrap()
}

fn is_failed(block: &pb::Block, transaction_index: u32) -> bool {
    block.transactions[transaction_index as usize].meta.as_ref().unwrap().err.is_some()
}

/// Asserts one projected operation against values cross-checked with the
/// Solana JSON-RPC `getBlock` (transaction order) and `getTransaction`
/// (`jsonParsed` instruction and token balances).
#[allow(clippy::too_many_arguments)]
fn assert_transfer_checked(
    operation: &spl_transfers::Operation,
    position: (u32, u32),
    signature: &str,
    amount: u64,
    mint: &str,
    source: &str,
    destination: &str,
    authority: &str,
    owners: (Option<&str>, Option<&str>),
) {
    assert_eq!((operation.transaction_index, operation.instruction_index), position);
    assert_eq!((operation.inner_instruction_index, operation.stack_height), (None, None));
    assert_eq!((operation.program, operation.kind), (TokenProgram::Token, OperationKind::TransferChecked));
    assert_eq!(b58(operation.signature), signature);
    assert_eq!((operation.amount, operation.decimals), (amount, Some(6)));
    assert_eq!(operation.mint.map(b58).as_deref(), Some(mint));
    assert_eq!(operation.source.map(b58).as_deref(), Some(source));
    assert_eq!(operation.destination.map(b58).as_deref(), Some(destination));
    assert_eq!(b58(operation.authority), authority);
    assert_eq!(operation.source_owner.map(b58).as_deref(), owners.0);
    assert_eq!(operation.destination_owner.map(b58).as_deref(), owners.1);
}

#[test]
fn zero_amount_transfer_checked_is_kept() {
    let block = decode_block(BLOCK_350000009);
    let projection = projection(&block, Coverage::TRANSFER_CHECKED);

    let clock = &projection.clock;
    assert_eq!((clock.slot, clock.parent_slot), (350000009, 350000008));
    assert_eq!(clock.block_time, Some(1751210684));
    assert_eq!(clock.transaction_count, 1451);
    assert_eq!(b58(clock.blockhash), block.blockhash);
    assert_eq!(b58(clock.previous_blockhash), block.previous_blockhash);
    assert_eq!(clock.block_height, block.block_height.as_ref().map(|height| height.block_height));
    assert_eq!(
        (clock.coverage, clock.spec_revision),
        (Coverage::TRANSFER_CHECKED, spl_transfers::SPEC_REVISION)
    );

    assert_eq!(projection.operations.len(), 1);
    let operation = &projection.operations[0];
    assert_transfer_checked(
        operation,
        (1366, 0),
        "x1amHVAjq9FJGvQDXoVZr9wMbudDxQCxJm6KP3HL5jYkGFAoqb9yqwHdPYHmVk9zseC1UmJ5Lttc5pDvbzw9Ffx",
        0,
        "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm",
        "5TeNCFDMFHg4g9e5bdhYRHbz4Xh3pVPKGPssCSPxXq3s",
        "77vy5mho7zbwYNEVMa5woGCQjou6hyf1Y1ZMFShh6ouq",
        "3pKWHSm8VvNSGB9emgY3rNc34aVAoxXQPCqTsjMbyRt2",
        (
            Some("3pKWHSm8VvNSGB9emgY3rNc34aVAoxXQPCqTsjMbyRt2"),
            Some("5ZCEq4uvFYtKGKKM6VRRuDEapndyCta9v2dHDH6cVhTa"),
        ),
    );
    assert!(operation.extra_accounts.is_empty());
}

#[test]
fn loaded_addresses_and_temporary_accounts() {
    let block = decode_block(BLOCK_350000026);
    let operations = projection(&block, Coverage::TRANSFER_CHECKED).operations;
    assert_eq!(operations.len(), 4);
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    // The USDC mint of this TransferChecked is a loaded readonly address.
    assert_transfer_checked(
        &operations[0],
        (776, 3),
        "5SzaWPqdh4Kehdae3h1jwiSpEgczuusJffSUtEWL33CXoPqMeCwZXb1XTVV9G6zzJuCTiyfJgZ7Bes59VpRH3NcQ",
        989999,
        USDC,
        "5yrcgUAs5JGYYr5kFZqsz13jzBiANCSRUNKqc4UXRWGJ",
        "2am4n8dNv4HrVUVpL4SFqmdzfRznyxrBx3SXwp8WZ9pR",
        "EsF1h2qZ84aWSPrWB2nVemqFxquYDi1RBa3aMqhqfcjJ",
        (
            Some("EsF1h2qZ84aWSPrWB2nVemqFxquYDi1RBa3aMqhqfcjJ"),
            Some("5wkyL2FLEcyUUgc3UeGntHTAfWfzDrVuxMnaMm7792Gk"),
        ),
    );
    let message = block.transactions[776].transaction.as_ref().unwrap().message.as_ref().unwrap();
    let mint_index = message.instructions[3].accounts[1] as usize;
    assert!(mint_index >= message.account_keys.len(), "mint resolved from a loaded address");
    assert!(!message.address_table_lookups.is_empty());

    // CaU2X... is created and closed inside the transaction: no token balance
    // records it, so its owner stays unknown.
    let temporary = "CaU2Xeck26cGamW6HGD3yJo6ZgJqusDqPVd4ABK3zyk9";
    let signature = "4DHno5JrSSv3ysMMciorJCfSRScbbi5MGaweqrckD15218CbRKt7kG5pFqX1WDM7XYzMMt53Qt2fLpiRZYNDXM4y";
    assert_transfer_checked(
        &operations[1],
        (1282, 1),
        signature,
        5100,
        USDC,
        "9P5s2ZhZwck46KLjGRKqYtxJSLMpswqpMP1F6c3FyQTT",
        temporary,
        "79WBQcB4yHboe7zxy3m2Xax2zauehTm1tE1xCxw77777",
        (Some("79WBQcB4yHboe7zxy3m2Xax2zauehTm1tE1xCxw77777"), None),
    );
    assert_transfer_checked(
        &operations[2],
        (1282, 2),
        signature,
        5100,
        USDC,
        temporary,
        "6ZWUGDdd4WurCuPQFQqfFTAkWBM4AziPPzMBdyxnaftN",
        "A1mKZnLTGg42X8QhZkHEpicZtg7dknGKWn8MkN5UHL3s",
        (None, Some("E9epoMw29t3RiKJjY4SN427v5UTUreVkA2Zf42pyQHwu")),
    );

    // Accounts after the authority are kept apart from it; here the sender
    // lists the authority a second time (confirmed with `getTransaction`).
    assert_transfer_checked(
        &operations[3],
        (1337, 2),
        "4sFqM35hezjsZA2m13DxQhFg6oVF9skwZYhg3NimqoXoHnwRTB5yADQqLQmrKRpE5qKMbBrhMPoPm6y8KJM4V617",
        33032,
        USDC,
        "GgdvxncGM7bSASLeMU47pKrMqr2NVLcJkhCKp1yVErYK",
        "E6esoedxQ6KEcN8NPq6fvftnNdeBSqWkFSvVqS7prhDw",
        "8CFsjcAjFUrB6jkjUdMzcZWfVF6rHpufAoqAYeb4uZ6R",
        (
            Some("8CFsjcAjFUrB6jkjUdMzcZWfVF6rHpufAoqAYeb4uZ6R"),
            Some("EvCLcbQZo275XuAnAM9R2QxmiiXtBhqh1Pn2qZh9hiiC"),
        ),
    );
    assert_eq!(operations[3].extra_accounts, vec![operations[3].authority]);
}

#[test]
fn self_transfers_are_kept() {
    let mut block = decode_block(BLOCK_350000026);

    // Real self transfer, from an inner instruction.
    let all = projection(&block, Coverage::ALL);
    let selfs: Vec<_> = all.operations.iter().filter(|operation| operation.source == operation.destination).collect();
    assert_eq!(selfs.len(), 1);
    assert_eq!(
        (selfs[0].transaction_index, selfs[0].instruction_index, selfs[0].inner_instruction_index),
        (1335, 0, Some(6))
    );
    assert_eq!((selfs[0].kind, selfs[0].amount), (OperationKind::TransferChecked, 4051));

    // Top-level self transfer: point the destination of 776.3 at its source.
    let instruction = &mut message_mut(&mut block, 776).instructions[3];
    instruction.accounts[2] = instruction.accounts[0];
    let operations = projection(&block, Coverage::TRANSFER_CHECKED).operations;
    assert_eq!(operations.len(), 4);
    assert_eq!(operations[0].source, operations[0].destination);
    assert_eq!(operations[0].source_owner, operations[0].destination_owner);
}

#[test]
fn failed_transactions_are_not_projected() {
    let block = decode_block(BLOCK_350000010);
    // Both failed transactions carry a top-level classic TransferChecked.
    for transaction_index in [22u32, 119] {
        assert!(is_failed(&block, transaction_index));
        let message = block.transactions[transaction_index as usize]
            .transaction
            .as_ref()
            .unwrap()
            .message
            .as_ref()
            .unwrap();
        assert!(message.instructions.iter().any(|instruction| {
            message.account_keys[instruction.program_id_index as usize] == spl_transfers::TOKEN_PROGRAM_ID && instruction.data.first() == Some(&12)
        }));
    }
    for coverage in [Coverage::TRANSFER_CHECKED, Coverage::ALL] {
        let projection = projection(&block, coverage);
        assert_eq!(projection.clock.transaction_count, 760);
        assert!(projection.operations.iter().all(|operation| !is_failed(&block, operation.transaction_index)));
    }
    assert!(projection(&block, Coverage::TRANSFER_CHECKED).operations.is_empty());
}

#[test]
fn skipped_slots_are_accepted() {
    let block = decode_block(BLOCK_349019252);
    let projection = projection(&block, Coverage::TRANSFER_CHECKED);
    // Slots 349019248..=349019251 were skipped.
    assert_eq!((projection.clock.slot, projection.clock.parent_slot), (349019252, 349019247));
    assert_eq!(projection.clock.transaction_count, 1773);
    assert_eq!(projection.operations.len(), 2);

    let all = self::projection(&block, Coverage::ALL);
    assert!(all.operations.iter().any(|operation| operation.program == TokenProgram::Token2022));
    assert!(projection.operations.iter().all(|operation| operation.program == TokenProgram::Token));
}

#[test]
fn missing_block_time_is_preserved() {
    let mut block = decode_block(BLOCK_350000009);
    let expected = projection(&block, Coverage::ALL).operations;
    block.block_time = None;
    let projection = projection(&block, Coverage::ALL);
    assert_eq!(projection.clock.block_time, None);
    assert_eq!(projection.operations, expected);
}

#[test]
fn empty_block_yields_a_clock() {
    let mut block = decode_block(BLOCK_350000009);
    block.transactions.clear();
    let projection = projection(&block, Coverage::ALL);
    assert_eq!(projection.clock.slot, 350000009);
    assert_eq!(projection.clock.transaction_count, 0);
    assert_eq!(b58(projection.clock.blockhash), block.blockhash);
    assert!(projection.operations.is_empty());
}

#[test]
fn malformed_account_index_is_an_error() {
    let mut block = decode_block(BLOCK_350000009);
    let key_count = {
        let transaction = &block.transactions[1366];
        transaction.transaction.as_ref().unwrap().message.as_ref().unwrap().account_keys.len()
            + transaction.meta.as_ref().unwrap().loaded_writable_addresses.len()
            + transaction.meta.as_ref().unwrap().loaded_readonly_addresses.len()
    };
    message_mut(&mut block, 1366).instructions[0].accounts[2] = 255;
    assert_eq!(
        transaction_error(&block),
        (
            1366,
            TransactionError::AccountIndexOutOfBounds {
                position: Position {
                    instruction_index: 0,
                    inner_instruction_index: None
                },
                account_position: 2,
                account_index: 255,
                key_count,
            }
        )
    );
}

#[test]
fn malformed_program_index_is_an_error() {
    let mut block = decode_block(BLOCK_350000009);
    message_mut(&mut block, 1366).instructions[0].program_id_index = 1000;
    assert!(matches!(
        transaction_error(&block),
        (1366, TransactionError::ProgramIndexOutOfBounds { program_id_index: 1000, .. })
    ));
}

#[test]
fn malformed_inner_instruction_index_is_an_error() {
    let mut block = decode_block(BLOCK_350000026);
    let inner = &mut block.transactions[1335].meta.as_mut().unwrap().inner_instructions[0];
    let index = inner.index;
    inner.instructions[6].accounts[0] = 255;
    assert!(matches!(
        transaction_error(&block),
        (1335, TransactionError::AccountIndexOutOfBounds { position, account_index: 255, .. })
            if position == Position { instruction_index: index, inner_instruction_index: Some(6) }
    ));

    let mut block = decode_block(BLOCK_350000026);
    let meta = block.transactions[1335].meta.as_mut().unwrap();
    meta.inner_instructions[0].index = 200;
    assert!(matches!(
        transaction_error(&block),
        (1335, TransactionError::InnerInstructionsOutOfBounds { index: 200, .. })
    ));

    let mut block = decode_block(BLOCK_350000026);
    let meta = block.transactions[1335].meta.as_mut().unwrap();
    let duplicate = meta.inner_instructions[0].clone();
    meta.inner_instructions.push(duplicate);
    assert!(matches!(transaction_error(&block), (1335, TransactionError::DuplicateInnerInstructions { .. })));
}

#[test]
fn malformed_loaded_addresses_are_an_error() {
    let mut block = decode_block(BLOCK_350000026);
    block.transactions[776].meta.as_mut().unwrap().loaded_readonly_addresses.pop();
    assert!(matches!(transaction_error(&block), (776, TransactionError::LoadedAddressCount { .. })));

    let mut block = decode_block(BLOCK_350000026);
    block.transactions[776].meta.as_mut().unwrap().loaded_readonly_addresses[0].truncate(31);
    assert!(matches!(transaction_error(&block), (776, TransactionError::InvalidAccountKey { len: 31, .. })));
}

#[test]
fn truncated_token_instruction_in_successful_transaction_is_an_error() {
    let mut block = decode_block(BLOCK_350000009);
    message_mut(&mut block, 1366).instructions[0].accounts.truncate(3);
    assert_eq!(
        transaction_error(&block),
        (
            1366,
            TransactionError::Decode {
                position: Position {
                    instruction_index: 0,
                    inner_instruction_index: None
                },
                error: DecodeError::MissingAccounts {
                    kind: OperationKind::TransferChecked,
                    required: 4,
                    found: 3
                },
            }
        )
    );

    let mut block = decode_block(BLOCK_350000009);
    message_mut(&mut block, 1366).instructions[0].data.truncate(9);
    assert!(matches!(
        transaction_error(&block),
        (
            1366,
            TransactionError::Decode {
                error: DecodeError::InvalidData { len: 9, .. },
                ..
            }
        )
    ));
}

#[test]
fn failed_transactions_are_only_checked_for_identity() {
    // Instruction-level damage in a failed transaction neither fails nor
    // changes the projection.
    let mut block = decode_block(BLOCK_350000010);
    let expected = projection(&block, Coverage::ALL);
    let message = message_mut(&mut block, 22);
    for instruction in &mut message.instructions {
        instruction.accounts.truncate(1);
        instruction.program_id_index = 255;
    }
    assert_eq!(projection(&block, Coverage::ALL), expected);

    // Identity is still required.
    block.transactions[22].transaction.as_mut().unwrap().signatures[0].truncate(10);
    assert_eq!(transaction_error(&block), (22, TransactionError::InvalidSignature { position: 0, len: 10 }));
}

#[test]
fn invalid_token_balance_owner_is_an_error_when_used() {
    let mut block = decode_block(BLOCK_350000009);
    for balance in &mut block.transactions[1366].meta.as_mut().unwrap().pre_token_balances {
        balance.owner = "not-base58!".to_string();
    }
    assert!(matches!(transaction_error(&block), (1366, TransactionError::InvalidTokenBalanceOwner { .. })));
}

#[test]
fn block_identity_is_validated() {
    let block = decode_block(BLOCK_350000009);

    let mut broken = block.clone();
    broken.blockhash = "abc".to_string();
    assert!(matches!(
        validate_block(&view_block(&broken)),
        Err(ProjectError::InvalidBlockHash { field: "blockhash", .. })
    ));

    let mut broken = block.clone();
    broken.previous_blockhash.clear();
    assert!(matches!(
        validate_block(&view_block(&broken)),
        Err(ProjectError::InvalidBlockHash {
            field: "previous_blockhash",
            ..
        })
    ));

    let mut broken = block.clone();
    broken.parent_slot = broken.slot;
    assert!(matches!(validate_block(&view_block(&broken)), Err(ProjectError::InvalidParentSlot { .. })));

    let mut broken = block.clone();
    broken.block_time = Some(pb::UnixTimestamp { timestamp: -1 });
    assert_eq!(validate_block(&view_block(&broken)), Err(ProjectError::NegativeBlockTime { block_time: -1 }));

    let mut broken = block.clone();
    broken.transactions[5].meta = None;
    assert_eq!(transaction_error(&broken), (5, TransactionError::MissingMeta));

    let mut broken = block.clone();
    broken.transactions[5].transaction.as_mut().unwrap().message = None;
    assert_eq!(transaction_error(&broken), (5, TransactionError::MissingMessage));

    let mut broken = block.clone();
    message_mut(&mut broken, 5).header = None;
    assert_eq!(transaction_error(&broken), (5, TransactionError::MissingHeader));

    let mut broken = block;
    broken.transactions[5].transaction.as_mut().unwrap().signatures.clear();
    assert!(matches!(
        transaction_error(&broken),
        (5, TransactionError::SignatureCount { signatures: 0, .. })
    ));
}

#[test]
fn every_real_fixture_validates() {
    for fixture in [BLOCK_349019252, BLOCK_350000009, BLOCK_350000010, BLOCK_350000026, BLOCK_461335700] {
        assert_eq!(validate_block(&view_block(&decode_block(fixture))), Ok(()));
    }
}

#[test]
fn coverage_flags_select_subsets_of_all() {
    for fixture in [BLOCK_349019252, BLOCK_350000026] {
        let block = decode_block(fixture);
        let all = projection(&block, Coverage::ALL).operations;
        for bits in 0u8..16 {
            let coverage = Coverage {
                token_2022: bits & 1 != 0,
                inner_instructions: bits & 2 != 0,
                unchecked_transfers: bits & 4 != 0,
                mint_burn: bits & 8 != 0,
            };
            let expected: Vec<_> = all
                .iter()
                .filter(|operation| coverage.includes(operation.program, operation.kind, operation.inner_instruction_index.is_some()))
                .cloned()
                .collect();
            assert_eq!(projection(&block, coverage).operations, expected, "{coverage:?}");
        }
        // Every class appears in the real blocks.
        assert!(all.iter().any(|operation| operation.inner_instruction_index.is_some()));
        assert!(all.iter().any(|operation| operation.kind == OperationKind::Transfer));
        assert!(all.iter().any(|operation| operation.kind == OperationKind::MintTo));
        if fixture == BLOCK_349019252 {
            assert!(all.iter().any(|operation| operation.program == TokenProgram::Token2022));
            assert!(all.iter().any(|operation| operation.kind == OperationKind::Burn));
        }
    }
}

#[test]
fn operations_follow_execution_order() {
    let block = decode_block(BLOCK_350000026);
    let operations = projection(&block, Coverage::ALL).operations;
    let positions: Vec<_> = operations
        .iter()
        .map(|operation| (operation.transaction_index, operation.instruction_index, operation.inner_instruction_index))
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    // Inner instructions carry their recorded stack height; outer ones none.
    assert!(operations
        .iter()
        .all(|operation| operation.inner_instruction_index.is_some() == operation.stack_height.is_some()));
    assert!(operations.iter().filter_map(|operation| operation.stack_height).all(|height| height >= 2));
}

#[test]
fn unchecked_transfers_have_no_mint() {
    let block = decode_block(BLOCK_350000026);
    let operations = projection(&block, Coverage::ALL).operations;
    let unchecked: Vec<_> = operations.iter().filter(|operation| operation.kind == OperationKind::Transfer).collect();
    assert!(!unchecked.is_empty());
    for operation in unchecked {
        assert_eq!((operation.mint, operation.decimals), (None, None));
        assert!(operation.source.is_some() && operation.destination.is_some());
        assert_ne!(operation.destination, Some(operation.authority));
    }
}

#[test]
fn coverage_parsing() {
    assert_eq!(Coverage::parse(""), Ok(Coverage::TRANSFER_CHECKED));
    assert_eq!(Coverage::parse(" "), Ok(Coverage::TRANSFER_CHECKED));
    assert_eq!(Coverage::parse("all"), Ok(Coverage::ALL));
    assert_eq!(
        Coverage::parse("token_2022,inner_instructions,unchecked_transfers,mint_burn"),
        Ok(Coverage::ALL)
    );
    assert_eq!(
        Coverage::parse("inner_instructions, mint_burn"),
        Ok(Coverage {
            inner_instructions: true,
            mint_burn: true,
            ..Coverage::TRANSFER_CHECKED
        })
    );
    assert!(Coverage::parse("cpi").is_err());
    for coverage in [Coverage::TRANSFER_CHECKED, Coverage::ALL, Coverage::parse("token_2022").unwrap()] {
        assert_eq!(Coverage::parse(&coverage.to_string()), Ok(coverage));
    }
}
