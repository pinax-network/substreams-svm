use crate::{map_events, native::Collector, pb, project, project_with_limits, InstructionPosition, Limits, ProjectError, Provenance, SwapKind};
use buffa::Message;
use flate2::read::GzDecoder;
use std::io::Read;
use substreams_solana::pb::sf::solana::r#type::v1 as source;

fn fixture() -> source::Block {
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../fixtures/solana-mainnet-350000026.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    source::Block::decode_from_slice(bytes.as_slice()).unwrap()
}

#[test]
fn complete_block_native_projection_preserves_legacy_fields_and_original_positions() {
    let block = fixture();
    let projection = project(&block).unwrap();
    assert!(!projection.transactions.is_empty());
    let mut instructions = 0;
    let mut logs = 0;
    let mut summaries = 0;
    for transaction in &projection.transactions {
        let original = &block.transactions[transaction.transaction_index as usize];
        assert!(original.meta.as_option().unwrap().err.is_unset());
        assert_eq!(transaction.signature, original.transaction.as_option().unwrap().signatures[0]);
        for row in &transaction.swaps {
            summaries += usize::from(row.kind == SwapKind::AggregatorSummary);
            match row.provenance {
                Provenance::Instruction { position, .. } => {
                    instructions += 1;
                    let message = original.transaction.as_option().unwrap().message.as_option().unwrap();
                    assert!((position.instruction_index as usize) < message.instructions.len());
                    if let Some(inner) = position.inner_instruction_index {
                        let group = original
                            .meta
                            .as_option()
                            .unwrap()
                            .inner_instructions
                            .iter()
                            .find(|group| group.index == position.instruction_index)
                            .unwrap();
                        assert!((inner as usize) < group.instructions.len());
                    }
                }
                Provenance::Log { log_index, .. } => {
                    logs += 1;
                    assert!((log_index as usize) < original.meta.as_option().unwrap().log_messages.len());
                }
            }
        }
    }
    assert!(instructions > 0 && logs > 0 && summaries > 0);
    assert_eq!(projection.clock.slot, 350000026);
    assert_eq!(projection.clock.transaction_count, block.transactions.len() as u32);
    let qualified = projection
        .transactions
        .iter()
        .flat_map(|tx| &tx.swaps)
        .filter(|row| {
            row.invocation_outcome == crate::InvocationOutcome::Succeeded && row.verified_context.is_some() && row.swap.protocol == pb::Protocol::MeteoraDlmm
        })
        .count();
    assert_eq!(qualified, 5);
    let native = projection.into_legacy();
    let wrapper = map_events(block).unwrap();
    assert_eq!(native, wrapper);
    assert_eq!(native.encode_to_vec(), wrapper.encode_to_vec());
    assert_eq!(native.transactions.len(), 156);
    assert_eq!(native.transactions.iter().map(|tx| tx.swaps.len()).sum::<usize>(), 231);
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(native.encode_to_vec())),
        "c1e611c718878724d851e446c0a13bec1b28e682ebf99e2ab7b4a2d221b78218"
    );
}

use sha2::Digest;

#[test]
fn empty_missing_time_and_skipped_slot_block_has_a_clock() {
    let mut block = fixture();
    block.transactions.clear();
    block.block_time = None.into();
    block.parent_slot = block.slot - 5;
    let projection = project(&block).unwrap();
    assert!(projection.transactions.is_empty());
    assert_eq!(projection.clock.block_time, None);
    assert_eq!(projection.clock.parent_slot, block.slot - 5);
}

#[test]
fn structural_failure_and_capacity_return_no_partial_projection() {
    let mut block = fixture();
    let success = block
        .transactions
        .iter()
        .position(|tx| tx.meta.as_option().is_some_and(|meta| meta.err.is_unset()))
        .unwrap();
    block.transactions[success]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .instructions
        .push(source::CompiledInstruction {
            program_id_index: u32::MAX,
            ..Default::default()
        });
    assert!(matches!(project(&block), Err(ProjectError::InvalidBlock(_))));
    let block = fixture();
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_transactions: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_instructions: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_log_messages: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_log_bytes: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_swaps: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
}

#[test]
fn collector_keeps_distinct_event_positions_and_aggregator_kind() {
    let mut collector = Collector::new(10);
    let position = InstructionPosition {
        instruction_index: 8,
        inner_instruction_index: Some(3),
    };
    collector.at_instruction(position);
    collector.push(pb::Swap {
        protocol: pb::Protocol::MeteoraDlmm.into(),
        ..Default::default()
    });
    collector.push(pb::Swap {
        protocol: pb::Protocol::JupiterV6.into(),
        ..Default::default()
    });
    collector.at_log(99);
    collector.push(pb::Swap {
        protocol: pb::Protocol::OkxDex.into(),
        ..Default::default()
    });
    let rows = collector.finish().unwrap();
    assert_eq!(rows[0].provenance, Provenance::Instruction { position, event_index: 0 });
    assert_eq!(rows[1].provenance, Provenance::Instruction { position, event_index: 1 });
    assert_eq!(rows[2].provenance, Provenance::Log { log_index: 99, event_index: 0 });
    assert_eq!(rows[0].kind, SwapKind::PoolExecution);
    assert_eq!(rows[1].kind, SwapKind::AggregatorSummary);
    assert_eq!(rows[2].kind, SwapKind::AggregatorSummary);
}

#[test]
fn native_context_rejects_stale_dlmm_fifo_after_caught_failure() {
    use crate::InvocationOutcome;
    let fixture = fixture();
    let projected = project(&fixture).unwrap();
    let (transaction, row) = projected
        .transactions
        .iter()
        .find_map(|tx| {
            tx.swaps
                .iter()
                .find(|row| row.verified_context.is_some() && row.swap.protocol == pb::Protocol::MeteoraDlmm)
                .map(|row| (tx, row))
        })
        .unwrap();
    let original = &fixture.transactions[transaction.transaction_index as usize];
    fn instruction(tx: &source::ConfirmedTransaction, position: InstructionPosition) -> source::InnerInstruction {
        if let Some(inner) = position.inner_instruction_index {
            tx.meta
                .as_option()
                .unwrap()
                .inner_instructions
                .iter()
                .find(|group| group.index == position.instruction_index)
                .unwrap()
                .instructions[inner as usize]
                .clone()
        } else {
            let ix = &tx.transaction.as_option().unwrap().message.as_option().unwrap().instructions[position.instruction_index as usize];
            source::InnerInstruction {
                program_id_index: ix.program_id_index,
                accounts: ix.accounts.clone(),
                data: ix.data.clone(),
                stack_height: None,
            }
        }
    }
    let Provenance::Instruction { position, .. } = row.provenance else {
        panic!("expected instruction evidence")
    };
    let mut parent = instruction(original, row.verified_context.unwrap());
    parent.stack_height = Some(2);
    let mut event = instruction(original, position);
    event.stack_height = Some(3);
    let mut previous = parent.clone();
    // All are valid account indices, but this previous swap has a different
    // context. It fails without emitting an event, leaving the legacy FIFO stale.
    previous.accounts.fill(parent.program_id_index as u8);
    let program = substreams_solana::base58::encode(&row.swap.program_id);
    // The last variant's outer invocation fails, which revokes the event's success.
    for (stale, failed) in [(false, false), (true, false), (false, true)] {
        let mut tx = original.clone();
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions = vec![source::CompiledInstruction {
            program_id_index: parent.program_id_index,
            ..Default::default()
        }];
        let mut instructions = Vec::new();
        let mut logs = vec![format!("Program {program} invoke [1]")];
        if stale {
            instructions.push(previous.clone());
            logs.extend([format!("Program {program} invoke [2]"), format!("Program {program} failed: caught error")]);
        }
        instructions.extend([parent.clone(), event.clone()]);
        logs.extend([
            format!("Program {program} invoke [2]"),
            format!("Program {program} invoke [3]"),
            format!("Program {program} success"),
            format!("Program {program} success"),
            if failed {
                format!("Program {program} failed: caught error")
            } else {
                format!("Program {program} success")
            },
        ]);
        let meta = tx.meta.as_option_mut().unwrap();
        meta.inner_instructions = vec![source::InnerInstructions { index: 0, instructions }];
        meta.log_messages = logs;
        let mut block = fixture.clone();
        block.transactions = vec![tx];
        let native = project(&block).unwrap();
        let rows = &native.transactions[0].swaps;
        assert_eq!(rows.len(), 1);
        let outcome = if failed { InvocationOutcome::Failed } else { InvocationOutcome::Succeeded };
        assert_eq!(rows[0].invocation_outcome, outcome);
        assert_eq!(rows[0].verified_context.is_some(), !stale && !failed);
        if stale {
            assert_eq!(rows[0].swap.amm_pool, row.swap.program_id);
            assert_ne!(rows[0].swap.amm_pool, row.swap.amm_pool);
        }
        assert_eq!(native.into_legacy(), map_events(block).unwrap());
    }
}

#[test]
fn native_projection_admits_v1_and_rejects_new_or_contradictory_transaction_versions() {
    let original = fixture();
    let expected = project(&original).unwrap();
    assert!(original.transactions[0].transaction.message.address_table_lookups.is_empty());
    // v1 resolves every account from its static keys; its inline compute
    // budget is not read, so the projection is unchanged.
    for configured in [false, true] {
        let mut block = original.clone();
        let message = block.transactions[0].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.versioned = true;
        message.version = Some(1);
        if configured {
            message.transaction_config = source::TransactionConfig {
                priority_fee: Some(1),
                compute_unit_limit: Some(200_000),
                ..Default::default()
            }
            .into();
        }
        assert_eq!(project(&block).unwrap(), expected);
    }
    for (version, versioned, lookups, configured) in [
        (Some(2), true, false, false),
        (Some(17), true, false, false),
        (Some(1), false, false, false),
        (Some(1), true, true, true),
        (Some(0), true, false, true),
        (None, false, false, true),
    ] {
        let mut block = original.clone();
        let message = block.transactions[0].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.versioned = versioned;
        message.version = version;
        if lookups {
            message.address_table_lookups.push(Default::default());
        }
        if configured {
            message.transaction_config = source::TransactionConfig::default().into();
        }
        assert!(
            matches!(project(&block), Err(ProjectError::UnsupportedMessage(_))),
            "{version:?} {versioned} {lookups} {configured}"
        );
    }
}

#[test]
fn current_block_with_v1_messages_matches_the_wrapper() {
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../../spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let block = source::Block::decode_from_slice(bytes.as_slice()).unwrap();
    let projection = project(&block).unwrap();
    let rows = |v1_only: bool| {
        projection
            .transactions
            .iter()
            .filter(|tx| !v1_only || block.transactions[tx.transaction_index as usize].transaction.message.version == Some(1))
            .map(|tx| tx.swaps.len())
            .sum::<usize>()
    };
    assert_eq!((rows(false), rows(true)), (61, 10));
    assert_eq!(projection.into_legacy(), map_events(block).unwrap());
}
