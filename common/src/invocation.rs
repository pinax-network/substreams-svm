//! Conservative invocation evidence shared by native and Substreams decoders.
//!
//! Callers validate complete transaction structure and transaction success before
//! using the trace to admit committed state. An exact invocation trace does not
//! independently validate account indexes or the transaction's overall outcome.
use std::collections::BTreeMap;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstructionPosition {
    pub instruction_index: u32,
    pub inner_instruction_index: Option<u32>,
}

/// Success requires an exact instruction/log invocation trace, including every
/// enclosing call. Missing, truncated, or ambiguous logs leave it unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationOutcome {
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evidence {
    pub outcome: InvocationOutcome,
    pub parent: Option<InstructionPosition>,
    /// Log index of this instruction's own `invoke [depth]` line. `None` only
    /// for a top-level signature-verification precompile, which the runtime
    /// verifies without an invocation log.
    pub invoke_log_index: Option<u32>,
}

/// An exact alignment of the recorded instruction walk with the runtime
/// invocation log grammar. Only produced when every invocation is matched.
pub struct Trace {
    pub evidence: BTreeMap<InstructionPosition, Evidence>,
    /// For every original log index, the invocation executing when the line
    /// was written (the innermost open invocation; for an outcome line, the
    /// invocation it closes). `None` for lines outside every invocation.
    pub emitters: Vec<Option<InstructionPosition>>,
}

/// Align every recorded instruction with its exact runtime invocation logs.
/// Returns `None` for incomplete, truncated, malformed, or ambiguous evidence.
/// The caller must separately validate transaction structure and success.
pub fn prove(tx: &ConfirmedTransaction) -> Option<Trace> {
    let meta = tx.meta.as_option()?;
    // Past the byte limit the runtime drops each line that does not fit and
    // writes this marker once, yet it still records later lines that fit. Lines
    // can then be missing anywhere after the marker while the remaining
    // invocation lines still balance, so no part of the trace is exact.
    if meta.log_messages.iter().any(|log| log == "Log truncated") {
        return None;
    }
    let expected = instruction_positions(tx)
        .into_iter()
        .zip(tx.walk_instructions())
        .map(|(position, instruction)| {
            let depth = if position.inner_instruction_index.is_none() {
                Some(1)
            } else {
                instruction.maybe_stack_height()
            };
            (position, instruction.program_id().0.to_vec(), depth)
        })
        .collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut parents: Vec<Option<usize>> = vec![None; expected.len()];
    let mut outcomes = vec![InvocationOutcome::Unknown; expected.len()];
    let mut invoke_lines: Vec<Option<u32>> = vec![None; expected.len()];
    let mut emitters: Vec<Option<usize>> = Vec::with_capacity(meta.log_messages.len());
    for (log_index, log) in meta.log_messages.iter().enumerate() {
        let Some((program, event)) = parse_log(log).ok()? else {
            emitters.push(stack.last().copied());
            continue;
        };
        if let Event::Invoke(depth) = event {
            if depth as usize != stack.len() + 1 {
                return None;
            }
            while let Some((position, candidate, candidate_depth)) = expected.get(cursor) {
                if candidate == &program && *candidate_depth == Some(depth) {
                    break;
                }
                if !without_logs(*position, candidate) {
                    return None;
                }
                cursor += 1;
            }
            let (position, candidate, candidate_depth) = expected.get(cursor)?;
            if candidate != &program || *candidate_depth != Some(depth) {
                return None;
            }
            // An inner instruction runs inside its own outer instruction. The
            // depth check alone would accept an inner instruction recorded at
            // height 1, or one of a skipped precompile's group, inside another
            // outer instruction's invocation.
            if position.inner_instruction_index.is_some() && stack.first().map(|&outer| expected[outer].0.instruction_index) != Some(position.instruction_index)
            {
                return None;
            }
            parents[cursor] = stack.last().copied();
            invoke_lines[cursor] = Some(u32::try_from(log_index).ok()?);
            stack.push(cursor);
            emitters.push(Some(cursor));
            cursor += 1;
        } else {
            let index = stack.pop()?;
            emitters.push(Some(index));
            if expected[index].1 != program {
                return None;
            }
            outcomes[index] = match event {
                Event::Success => InvocationOutcome::Succeeded,
                Event::Failure => InvocationOutcome::Failed,
                Event::Invoke(_) => unreachable!(),
            };
        }
    }
    if !stack.is_empty() {
        return None;
    }
    while let Some((position, candidate, _)) = expected.get(cursor) {
        if !without_logs(*position, candidate) {
            return None;
        }
        cursor += 1;
    }
    // A successful event self-CPI can still be rolled back by an enclosing
    // invocation that fails and is caught. Propagate every ancestor outcome.
    for index in 0..outcomes.len() {
        if let Some(parent) = parents[index] {
            outcomes[index] = match (outcomes[index], outcomes[parent]) {
                (_, InvocationOutcome::Failed) => InvocationOutcome::Failed,
                (_, InvocationOutcome::Unknown) => InvocationOutcome::Unknown,
                (outcome, InvocationOutcome::Succeeded) => outcome,
            };
        }
    }
    Some(Trace {
        evidence: expected
            .iter()
            .enumerate()
            .map(|(index, (position, _, _))| {
                (
                    *position,
                    Evidence {
                        outcome: outcomes[index],
                        parent: parents[index].map(|parent| expected[parent].0),
                        invoke_log_index: invoke_lines[index],
                    },
                )
            })
            .collect(),
        emitters: emitters.into_iter().map(|index| index.map(|index| expected[index].0)).collect(),
    })
}

/// Positions in the same order as the Solana SDK's instruction walk.
///
/// Callers must validate complete transaction structure before using positions
/// as evidence, including rejecting duplicate inner-instruction parent groups.
pub fn instruction_positions(tx: &ConfirmedTransaction) -> Vec<InstructionPosition> {
    let Some(message) = tx.transaction.as_option().and_then(|tx| tx.message.as_option()) else {
        return Vec::new();
    };
    // Match the SDK walk's last-entry behavior without a repeated linear scan.
    // The validated native API rejects duplicate parent entries beforehand.
    let inner_by_outer: std::collections::HashMap<_, _> = tx
        .meta
        .as_option()
        .into_iter()
        .flat_map(|meta| &meta.inner_instructions)
        .map(|group| (group.index, group))
        .collect();
    let mut positions = Vec::new();
    for index in 0..message.instructions.len() {
        positions.push(InstructionPosition {
            instruction_index: index as u32,
            inner_instruction_index: None,
        });
        if let Some(group) = inner_by_outer.get(&(index as u32)) {
            for inner in 0..group.instructions.len() {
                positions.push(InstructionPosition {
                    instruction_index: index as u32,
                    inner_instruction_index: Some(inner as u32),
                });
            }
        }
    }
    positions
}

#[derive(Clone, Copy)]
enum Event {
    Invoke(u32),
    Success,
    Failure,
}

/// Runtime invocation grammar must be exact. Protocol decoder helpers are
/// deliberately permissive and cannot serve as execution-success evidence.
fn parse_log(log: &str) -> Result<Option<(Vec<u8>, Event)>, ()> {
    let Some(rest) = log.strip_prefix("Program ") else { return Ok(None) };
    let Some((identity, tail)) = rest.split_once(' ') else { return Ok(None) };
    if matches!(identity, "log:" | "data:" | "return:") {
        return Ok(None);
    }
    let event = if tail == "success" {
        Event::Success
    } else if tail.strip_prefix("failed: ").is_some_and(|message| !message.is_empty()) {
        // The failure message may itself end in the word "success".
        Event::Failure
    } else if let Some(depth) = tail.strip_prefix("invoke [").and_then(|rest| rest.strip_suffix(']')) {
        let parsed: u32 = depth.parse().map_err(|_| ())?;
        if parsed == 0 || parsed.to_string() != depth {
            return Err(());
        }
        Event::Invoke(parsed)
    } else if tail.starts_with("invoke") || tail.starts_with("success") || tail.starts_with("failed") {
        return Err(());
    } else {
        return Ok(None);
    };
    let program = base58::decode(identity).map_err(|_| ())?;
    if program.len() != 32 || base58::encode(&program) != identity {
        return Err(());
    }
    Ok(Some((program, event)))
}

fn without_logs(position: InstructionPosition, program: &[u8]) -> bool {
    // Top-level signature-verification precompiles are checked without an
    // invocation log. Every other executed instruction, ComputeBudget included,
    // logs `invoke`; a successful transaction executes all of them. Never skip
    // an arbitrary program to force alignment.
    position.inner_instruction_index.is_none()
        && matches!(
            base58::encode(program).as_str(),
            "Ed25519SigVerify111111111111111111111111111" | "KeccakSecp256k11111111111111111111111111111" | "Secp256r1SigVerify1111111111111111111111111"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1 as pb;

    fn transaction() -> pb::ConfirmedTransaction {
        let a = [1; 32];
        let b = [2; 32];
        let c = [3; 32];
        let logs = vec![
            format!("Program {} invoke [1]", base58::encode(a)),
            format!("Program {} invoke [2]", base58::encode(b)),
            format!("Program {} invoke [3]", base58::encode(c)),
            format!("Program {} success", base58::encode(c)),
            format!("Program {} success", base58::encode(b)),
            format!("Program {} success", base58::encode(a)),
        ];
        pb::ConfirmedTransaction {
            transaction: Some(pb::Transaction {
                signatures: vec![vec![1; 64]],
                message: Some(pb::Message {
                    header: Some(pb::MessageHeader {
                        num_required_signatures: 1,
                        ..Default::default()
                    })
                    .into(),
                    account_keys: vec![a.to_vec(), b.to_vec(), c.to_vec()],
                    instructions: vec![pb::CompiledInstruction {
                        program_id_index: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .into(),
            })
            .into(),
            meta: Some(pb::TransactionStatusMeta {
                inner_instructions: vec![pb::InnerInstructions {
                    index: 0,
                    instructions: vec![
                        pb::InnerInstruction {
                            program_id_index: 1,
                            stack_height: Some(2),
                            ..Default::default()
                        },
                        pb::InnerInstruction {
                            program_id_index: 2,
                            stack_height: Some(3),
                            ..Default::default()
                        },
                    ],
                }],
                log_messages: logs,
                ..Default::default()
            })
            .into(),
        }
    }

    #[test]
    fn successful_trace_proves_each_original_instruction() {
        let result = prove(&transaction()).unwrap().evidence;
        assert_eq!(result.len(), 3);
        assert!(result.values().all(|outcome| outcome.outcome == InvocationOutcome::Succeeded));
    }

    #[test]
    fn every_line_belongs_to_the_innermost_open_invocation() {
        let mut tx = transaction();
        // A data line written by `b` after its child `c` returned.
        tx.meta.as_option_mut().unwrap().log_messages.insert(4, "Program data: AAAA".to_string());
        let trace = prove(&tx).unwrap();
        let a = InstructionPosition {
            instruction_index: 0,
            inner_instruction_index: None,
        };
        let b = InstructionPosition {
            instruction_index: 0,
            inner_instruction_index: Some(0),
        };
        let c = InstructionPosition {
            instruction_index: 0,
            inner_instruction_index: Some(1),
        };
        assert_eq!(trace.emitters, [Some(a), Some(b), Some(c), Some(c), Some(b), Some(b), Some(a)]);
        assert_eq!(trace.evidence[&a].invoke_log_index, Some(0));
        assert_eq!(trace.evidence[&c].invoke_log_index, Some(2));
    }

    #[test]
    fn truncation_marker_rejects_an_otherwise_balanced_trace() {
        let mut tx = transaction();
        tx.meta.as_option_mut().unwrap().log_messages.push("Log truncated".to_string());
        assert!(prove(&tx).is_none());
    }

    #[test]
    fn caught_parent_failure_revokes_preceding_successful_event() {
        let mut tx = transaction();
        tx.meta.as_option_mut().unwrap().log_messages[4] = format!("Program {} failed: caught inner call ended with success", base58::encode([2; 32]));
        let result = prove(&tx).unwrap().evidence;
        assert_eq!(
            result[&InstructionPosition {
                instruction_index: 0,
                inner_instruction_index: None
            }]
                .outcome,
            InvocationOutcome::Succeeded
        );
        assert_eq!(
            result[&InstructionPosition {
                instruction_index: 0,
                inner_instruction_index: Some(1)
            }]
                .outcome,
            InvocationOutcome::Failed
        );
    }

    /// Append a top-level instruction of `program` that writes no log line.
    fn with_silent_instruction(mut tx: pb::ConfirmedTransaction, program: &str) -> pb::ConfirmedTransaction {
        let message = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.account_keys.push(base58::decode(program).unwrap());
        message.instructions.push(pb::CompiledInstruction {
            program_id_index: message.account_keys.len() as u32 - 1,
            ..Default::default()
        });
        tx
    }

    const ED25519: &str = "Ed25519SigVerify111111111111111111111111111";

    #[test]
    fn truncated_unknown_depth_and_misaligned_traces_never_prove_success() {
        // Control: a signature-verification precompile writes no invocation
        // log and is the only instruction alignment may skip.
        assert!(prove(&with_silent_instruction(transaction(), ED25519)).is_some());
        for mutation in 0..6 {
            let mut tx = transaction();
            match mutation {
                0 => {
                    tx.meta.as_option_mut().unwrap().log_messages.pop();
                }
                1 => tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[0].stack_height = None,
                2 => tx.meta.as_option_mut().unwrap().log_messages[1] = format!("Program {} invoke [2]", base58::encode([9; 32])),
                3 => {
                    // Inner 0.0 recorded at height 1 and logged after outer 0
                    // closed: depths align, but it ran outside its outer call.
                    let meta = tx.meta.as_option_mut().unwrap();
                    meta.inner_instructions[0].instructions[0].stack_height = Some(1);
                    meta.inner_instructions[0].instructions[1].stack_height = Some(2);
                    let (a, b, c) = (base58::encode([1; 32]), base58::encode([2; 32]), base58::encode([3; 32]));
                    meta.log_messages = vec![
                        format!("Program {a} invoke [1]"),
                        format!("Program {a} success"),
                        format!("Program {b} invoke [1]"),
                        format!("Program {c} invoke [2]"),
                        format!("Program {c} success"),
                        format!("Program {b} success"),
                    ];
                }
                4 => {
                    // The inner group belongs to the skipped precompile, so its
                    // calls cannot have run inside outer 0.
                    tx = with_silent_instruction(tx, ED25519);
                    tx.meta.as_option_mut().unwrap().inner_instructions[0].index = 1;
                }
                // ComputeBudget always logs its invocation when executed.
                5 => tx = with_silent_instruction(tx, "ComputeBudget111111111111111111111111111111"),
                _ => unreachable!(),
            }
            assert!(prove(&tx).is_none(), "mutation {mutation}");
        }
    }

    fn logs(tx: &mut pb::ConfirmedTransaction) -> &mut Vec<String> {
        &mut tx.meta.as_option_mut().unwrap().log_messages
    }

    /// Append a top-level instruction of `program` that logs its invocation.
    fn with_logged_instruction(mut tx: pb::ConfirmedTransaction, program: [u8; 32]) -> pb::ConfirmedTransaction {
        let message = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.account_keys.push(program.to_vec());
        message.instructions.push(pb::CompiledInstruction {
            program_id_index: message.account_keys.len() as u32 - 1,
            ..Default::default()
        });
        let key = base58::encode(program);
        logs(&mut tx).extend([format!("Program {key} invoke [1]"), format!("Program {key} success")]);
        tx
    }

    fn position(instruction_index: u32, inner_instruction_index: Option<u32>) -> InstructionPosition {
        InstructionPosition {
            instruction_index,
            inner_instruction_index,
        }
    }

    #[test]
    fn invoke_depth_must_follow_the_open_invocation_stack() {
        let (a, b, c, d) = (
            base58::encode([1; 32]),
            base58::encode([2; 32]),
            base58::encode([3; 32]),
            base58::encode([4; 32]),
        );
        // Control: top-level d logged after a closed proves, with each call
        // under the invocation that was open when it was logged.
        let evidence = prove(&with_logged_instruction(transaction(), [4; 32])).unwrap().evidence;
        assert_eq!(evidence[&position(0, Some(1))].parent, Some(position(0, Some(0))));
        assert_eq!(evidence[&position(1, None)].parent, None);
        assert_eq!(evidence[&position(1, None)].outcome, InvocationOutcome::Succeeded);

        // c is recorded at height 3 and logged at depth 3, but directly under
        // a after b closed: the line's depth disagrees with the open stack, so
        // accepting it would make a, not b, its parent.
        let mut tx = transaction();
        *logs(&mut tx) = vec![
            format!("Program {a} invoke [1]"),
            format!("Program {b} invoke [2]"),
            format!("Program {b} success"),
            format!("Program {c} invoke [3]"),
            format!("Program {c} success"),
            format!("Program {a} success"),
        ];
        assert!(prove(&tx).is_none(), "inner depth must follow the open stack");

        // Top-level d logged at depth 1 inside a's invocation.
        let mut tx = with_logged_instruction(transaction(), [4; 32]);
        *logs(&mut tx) = vec![
            format!("Program {a} invoke [1]"),
            format!("Program {b} invoke [2]"),
            format!("Program {c} invoke [3]"),
            format!("Program {c} success"),
            format!("Program {b} success"),
            format!("Program {d} invoke [1]"),
            format!("Program {d} success"),
            format!("Program {a} success"),
        ];
        assert!(prove(&tx).is_none(), "a top-level instruction cannot run inside another");
    }

    #[test]
    fn alignment_skips_only_top_level_precompiles_anywhere_in_the_sequence() {
        // Control: a silent precompile between two logged instructions proves.
        assert!(prove(&with_logged_instruction(with_silent_instruction(transaction(), ED25519), [4; 32])).is_some());

        // A silent ComputeBudget before a logged instruction: alignment must
        // not skip it to reach the next match (the trailing case is mutation 5
        // of truncated_unknown_depth_and_misaligned_traces_never_prove_success).
        let tx = with_logged_instruction(with_silent_instruction(transaction(), "ComputeBudget111111111111111111111111111111"), [4; 32]);
        assert!(prove(&tx).is_none(), "only precompiles may be skipped mid-sequence");

        // A silent precompile recorded as an inner instruction.
        let mut tx = transaction();
        let message = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.account_keys.push(base58::decode(ED25519).unwrap());
        let program_id_index = message.account_keys.len() as u32 - 1;
        tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions.push(pb::InnerInstruction {
            program_id_index,
            stack_height: Some(2),
            ..Default::default()
        });
        assert!(prove(&tx).is_none(), "only top-level precompiles run without logs");
    }

    #[test]
    fn outcome_lines_close_their_own_program_and_malformed_lines_reject_the_trace() {
        let (a, b) = (base58::encode([1; 32]), base58::encode([2; 32]));
        // Outcome lines naming the wrong program for the frame they close.
        let mut tx = transaction();
        logs(&mut tx)[4] = format!("Program {a} success");
        logs(&mut tx)[5] = format!("Program {b} success");
        assert!(prove(&tx).is_none(), "an outcome line closes its own program");

        // A malformed runtime line rejects the whole trace, not just itself.
        let mut tx = transaction();
        logs(&mut tx).insert(4, format!("Program {b} success extra"));
        assert!(prove(&tx).is_none(), "malformed runtime grammar must reject the trace");
    }
}

#[cfg(test)]
mod exact_log_tests {
    use super::*;
    #[test]
    fn failure_suffix_cannot_be_mistaken_for_success() {
        let key = base58::encode([2; 32]);
        assert!(matches!(
            parse_log(&format!("Program {key} failed: custom error ending in success")).unwrap(),
            Some((_, Event::Failure))
        ));
    }
    #[test]
    fn malformed_runtime_grammar_is_rejected() {
        let key = base58::encode([2; 32]);
        for tail in ["invoke [2] extra", "invoke [02]", "invoke [0]", "success extra", "failed:", "failed: "] {
            assert!(parse_log(&format!("Program {key} {tail}")).is_err());
        }
        assert!(parse_log("Program log: Program fake success").unwrap().is_none());
    }
}
