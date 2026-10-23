//! Bounded system-program instruction projection from an already decoded block.
use crate::{is_system_program, pb, system};
use std::fmt;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction};

spl_transfers::block_view_buffa!(fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_transactions: usize,
    pub max_instructions: usize,
    pub max_operations: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_transactions: 100_000,
            max_instructions: 100_000,
            max_operations: 100_000,
        }
    }
}

/// An observed instruction in a successful transaction. Inner invocations may
/// still have failed and been caught by their caller: this API does not prove
/// their success or equate requested lamports with a persisted balance change.
#[derive(Debug)]
pub struct Operation<'a> {
    pub transaction_index: u32,
    pub instruction_index: u32,
    pub inner_instruction_index: Option<u32>,
    pub transaction: &'a ConfirmedTransaction,
    pub instruction: pb::Instruction,
}

#[derive(Debug)]
pub struct Projection<'a> {
    /// Original header and transaction metadata, without another decode/copy.
    pub block: &'a Block,
    pub operations: Vec<Operation<'a>>,
}

#[derive(Debug)]
pub enum ProjectError {
    Capacity,
    InvalidBlock(spl_transfers::ProjectError),
    UnsupportedMessage(spl_transfers::UnsupportedMessage),
    InvalidSystemInstruction {
        transaction_index: u32,
        instruction_index: u32,
        inner_instruction_index: Option<u32>,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Capacity => "system projection capacity exceeded",
            Self::InvalidBlock(_) => "invalid complete Solana block",
            Self::UnsupportedMessage(_) => "unsupported Solana message",
            Self::InvalidSystemInstruction { .. } => "invalid system instruction",
        })
    }
}
impl std::error::Error for ProjectError {}

/// Decode positive-lamport Transfer, TransferWithSeed, CreateAccount,
/// CreateAccountWithSeed and WithdrawNonceAccount instructions, including CPI
/// and v0 loaded accounts. Failed transactions are omitted. Fees, rewards,
/// stake operations and arbitrary balance deltas are not system transfers.
/// Unknown/malformed system instructions fail closed; zero amounts are omitted
/// as in the legacy wrapper. Callers must also bound protobuf input bytes and
/// allocations before passing a decoded block here.
pub fn project(block: &Block) -> Result<Projection<'_>, ProjectError> {
    project_with_limits(block, Limits::default())
}

pub fn project_with_limits(block: &Block, limits: Limits) -> Result<Projection<'_>, ProjectError> {
    if block.transactions.len() > limits.max_transactions || block.transactions.len() > u32::MAX as usize {
        return Err(ProjectError::Capacity);
    }
    let mut count = 0usize;
    for tx in &block.transactions {
        count = count.saturating_add(tx.transaction.message.instructions.len());
        for group in &tx.meta.inner_instructions {
            count = count.saturating_add(group.instructions.len());
        }
        if count > limits.max_instructions || count > u32::MAX as usize {
            return Err(ProjectError::Capacity);
        }
    }
    let view = view_block(block).map_err(ProjectError::UnsupportedMessage)?;
    spl_transfers::validate_block(&view).map_err(ProjectError::InvalidBlock)?;
    let mut operations = Vec::new();
    for (transaction_index, tx) in block.transactions.iter().enumerate() {
        if !tx.is_successful() {
            continue;
        }
        // The SDK walks each outer instruction followed by its recorded CPIs.
        let mut outer = 0u32;
        let mut inner = 0u32;
        for instruction in tx.walk_instructions() {
            let (instruction_index, inner_instruction_index) = if instruction.is_root() {
                let index = outer;
                outer += 1;
                inner = 0;
                (index, None)
            } else {
                let index = inner;
                inner += 1;
                (outer - 1, Some(index))
            };
            if !is_system_program(instruction.program_id().0) {
                continue;
            }
            let decoded = system::try_unpack_transfers(&instruction).map_err(|_| ProjectError::InvalidSystemInstruction {
                transaction_index: transaction_index as u32,
                instruction_index,
                inner_instruction_index,
            })?;
            if let Some(decoded) = decoded {
                if operations.len() >= limits.max_operations {
                    return Err(ProjectError::Capacity);
                }
                operations.push(Operation {
                    transaction_index: transaction_index as u32,
                    instruction_index,
                    inner_instruction_index,
                    transaction: tx,
                    instruction: pb::Instruction {
                        program_id: crate::SYSTEM_PROGRAM.to_vec(),
                        stack_height: instruction.stack_height(),
                        is_root: instruction.is_root(),
                        instruction: Some(decoded),
                    },
                });
            }
        }
    }
    Ok(Projection { block, operations })
}
