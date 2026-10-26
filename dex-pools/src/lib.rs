//! Pool creation projection, with no Substreams execution, store or RPC calls.
//! [`project`] admits only matched initialization/event evidence in a completely
//! proved successful invocation trace. [`decode_pool_creation`] returns an
//! unverified candidate for callers that own a different transport model.
mod decode;

pub use common::invocation::InstructionPosition;
pub use decode::decode_pool_creation;
pub use substreams_solana_core as solana;
pub use substreams_solana_core::pb::sf::solana::r#type::v1::Block;

use base64::Engine;
use common::invocation::{self, InvocationOutcome};
use std::{collections::BTreeMap, fmt};
use substreams_solana_idls::{orca::whirlpool, pumpfun::amm};

spl_transfers::block_view_buffa!(fn view_block, solana::pb::sf::solana::r#type::v1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    PumpfunAmm,
    OrcaWhirlpool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub mint: [u8; 32],
    pub vault: Option<[u8; 32]>,
    pub decimals: Option<u8>,
    pub token_program: Option<[u8; 32]>,
}

/// Matched typed initialization and event fields, without execution proof.
/// Never write this directly to a canonical store: the transaction and every
/// enclosing invocation must also have committed successfully.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub protocol: Protocol,
    pub address: [u8; 32],
    pub program_id: [u8; 32],
    /// Protocol order (Pump base/quote, Orca A/B), never swap direction.
    pub tokens: [Token; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidencePosition {
    Instruction(InstructionPosition),
    Log { log_index: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creation {
    pub slot: u64,
    pub blockhash: String,
    /// Index in the supplied block. A no-vote input has already removed votes;
    /// this is not a promise of an index in the original unfiltered block.
    pub transaction_index: u32,
    pub signature: [u8; 64],
    pub instruction: InstructionPosition,
    pub evidence: EvidencePosition,
}

/// A creation admitted by [`project`] after complete structural and invocation
/// validation. The public fields are data; constructing a struct is not proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pool {
    pub protocol: Protocol,
    pub address: [u8; 32],
    pub program_id: [u8; 32],
    pub tokens: [Token; 2],
    pub creation: Creation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    pub slot: u64,
    pub blockhash: String,
    /// Sorted by raw pool address. Identical observations retain their first
    /// source position; conflicting metadata rejects the entire projection.
    pub pools: Vec<Pool>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_transactions: usize,
    pub max_instructions: usize,
    pub max_log_messages: usize,
    pub max_log_bytes: usize,
    /// Bytes in keys, signatures, instruction payloads and token identifiers,
    /// checked before allocating a structural view or invocation trace.
    pub max_input_bytes: usize,
    /// Aggregate key, signature, lookup, account-reference and balance entries.
    /// Also bounds malformed empty entries that consume no payload bytes.
    pub max_account_entries: usize,
    pub max_pools: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_transactions: 100_000,
            max_instructions: 100_000,
            max_log_messages: 1_000_000,
            max_log_bytes: 64 * 1024 * 1024,
            max_input_bytes: 256 * 1024 * 1024,
            max_account_entries: 2_000_000,
            max_pools: 10_000,
        }
    }
}

#[derive(Debug)]
pub enum ProjectError {
    Capacity,
    InvalidBlock(spl_transfers::ProjectError),
    UnsupportedMessage(spl_transfers::UnsupportedMessage),
    ConflictingPool { address: [u8; 32] },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => f.write_str("pool projection capacity exceeded"),
            Self::InvalidBlock(_) => f.write_str("invalid Solana block"),
            Self::UnsupportedMessage(error) => write!(f, "unsupported Solana message: {error}"),
            Self::ConflictingPool { address } => write!(f, "conflicting pool metadata for {}", solana::base58::encode(address)),
        }
    }
}
impl std::error::Error for ProjectError {}

pub fn project(block: &Block) -> Result<Projection, ProjectError> {
    project_with_limits(block, Limits::default())
}

pub fn project_with_limits(block: &Block, limits: Limits) -> Result<Projection, ProjectError> {
    check_limits(block, limits)?;
    let view = view_block(block).map_err(ProjectError::UnsupportedMessage)?;
    spl_transfers::validate_block(&view).map_err(ProjectError::InvalidBlock)?;
    let mut pools = BTreeMap::<[u8; 32], Pool>::new();
    for (transaction_index, tx) in block.transactions.iter().enumerate() {
        if !tx.is_successful() {
            continue;
        }
        // Avoid allocating a full trace for transactions outside our coverage.
        if !tx
            .walk_instructions()
            .any(|ix| ix.program_id().0.as_slice() == amm::PROGRAM_ID || ix.program_id().0.as_slice() == whirlpool::PROGRAM_ID)
        {
            continue;
        }
        let Some(trace) = invocation::prove(tx) else { continue };
        let instructions: BTreeMap<_, _> = invocation::instruction_positions(tx).into_iter().zip(tx.walk_instructions()).collect();
        let mut admit = |candidate: Candidate, instruction, evidence| -> Result<(), ProjectError> {
            if let Some(previous) = pools.get(&candidate.address) {
                if previous.protocol != candidate.protocol || previous.program_id != candidate.program_id || previous.tokens != candidate.tokens {
                    return Err(ProjectError::ConflictingPool { address: candidate.address });
                }
                return Ok(());
            }
            if pools.len() >= limits.max_pools {
                return Err(ProjectError::Capacity);
            }
            let pool = Pool {
                protocol: candidate.protocol,
                address: candidate.address,
                program_id: candidate.program_id,
                tokens: candidate.tokens,
                creation: Creation {
                    slot: block.slot,
                    blockhash: block.blockhash.clone(),
                    transaction_index: transaction_index as u32,
                    signature: tx.hash().try_into().expect("validated signature"),
                    instruction,
                    evidence,
                },
            };
            pools.insert(pool.address, pool);
            Ok(())
        };
        // Pump emits Anchor event self-CPIs. Only the actual immediate parent
        // initialization can supply its accounts; never use FIFO/global state.
        for (position, event_ix) in &instructions {
            if event_ix.program_id().0.as_slice() != amm::PROGRAM_ID {
                continue;
            }
            let Some(event) = trace.evidence.get(position).filter(|event| event.outcome == InvocationOutcome::Succeeded) else {
                continue;
            };
            let Some(parent) = event.parent else { continue };
            let Some(init) = instructions.get(&parent) else { continue };
            if init.program_id().0.as_slice() != amm::PROGRAM_ID {
                continue;
            }
            if let Some(candidate) = decode::from_view(init, event_ix.data()) {
                admit(candidate, parent, EvidencePosition::Instruction(*position))?;
            }
        }
        // Orca writes a PoolInitialized log in the initialization invocation.
        let meta = tx.meta.as_option().expect("validated metadata");
        for (index, line) in meta.log_messages.iter().enumerate() {
            let Some(encoded) = line.strip_prefix("Program data: ") else { continue };
            // Our fixed-size creation events are well below this bound.
            if encoded.len() > 4096 {
                continue;
            }
            let Some(position) = trace.emitters[index] else { continue };
            if !trace.evidence.get(&position).is_some_and(|event| event.outcome == InvocationOutcome::Succeeded) {
                continue;
            }
            let Some(init) = instructions.get(&position) else { continue };
            if init.program_id().0.as_slice() != whirlpool::PROGRAM_ID {
                continue;
            }
            let Ok(data) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
                continue;
            };
            if let Some(candidate) = decode::from_view(init, &data) {
                admit(candidate, position, EvidencePosition::Log { log_index: index as u32 })?;
            }
        }
    }
    Ok(Projection {
        slot: block.slot,
        blockhash: block.blockhash.clone(),
        pools: pools.into_values().collect(),
    })
}

fn check_limits(block: &Block, limits: Limits) -> Result<(), ProjectError> {
    if block.transactions.len() > limits.max_transactions || block.transactions.len() > u32::MAX as usize {
        return Err(ProjectError::Capacity);
    }
    let (mut instructions, mut logs, mut log_bytes, mut input_bytes, mut entries) = (0usize, 0usize, 0usize, 0usize, 0usize);
    input_bytes = input_bytes.saturating_add(block.blockhash.len()).saturating_add(block.previous_blockhash.len());
    if input_bytes > limits.max_input_bytes {
        return Err(ProjectError::Capacity);
    }
    for tx in &block.transactions {
        if let Some(transaction) = tx.transaction.as_option() {
            entries = entries.saturating_add(transaction.signatures.len());
            for signature in &transaction.signatures {
                input_bytes = input_bytes.saturating_add(signature.len());
            }
        }
        if let Some(message) = tx.transaction.as_option().and_then(|tx| tx.message.as_option()) {
            instructions = instructions.saturating_add(message.instructions.len());
            entries = entries
                .saturating_add(message.account_keys.len())
                .saturating_add(message.address_table_lookups.len());
            input_bytes = input_bytes.saturating_add(message.recent_blockhash.len());
            for key in &message.account_keys {
                input_bytes = input_bytes.saturating_add(key.len());
            }
            for lookup in &message.address_table_lookups {
                input_bytes = input_bytes
                    .saturating_add(lookup.account_key.len())
                    .saturating_add(lookup.writable_indexes.len())
                    .saturating_add(lookup.readonly_indexes.len());
            }
            for instruction in &message.instructions {
                entries = entries.saturating_add(instruction.accounts.len());
                input_bytes = input_bytes.saturating_add(instruction.data.len()).saturating_add(instruction.accounts.len());
            }
        }
        if let Some(meta) = tx.meta.as_option() {
            entries = entries
                .saturating_add(meta.loaded_writable_addresses.len())
                .saturating_add(meta.loaded_readonly_addresses.len())
                .saturating_add(meta.pre_token_balances.len())
                .saturating_add(meta.post_token_balances.len())
                .saturating_add(meta.inner_instructions.len());
            for key in meta.loaded_writable_addresses.iter().chain(&meta.loaded_readonly_addresses) {
                input_bytes = input_bytes.saturating_add(key.len());
            }
            for balance in meta.pre_token_balances.iter().chain(&meta.post_token_balances) {
                input_bytes = input_bytes
                    .saturating_add(balance.mint.len())
                    .saturating_add(balance.owner.len())
                    .saturating_add(balance.program_id.len());
            }
            for group in &meta.inner_instructions {
                instructions = instructions.saturating_add(group.instructions.len());
                for instruction in &group.instructions {
                    entries = entries.saturating_add(instruction.accounts.len());
                    input_bytes = input_bytes.saturating_add(instruction.data.len()).saturating_add(instruction.accounts.len());
                }
            }
            logs = logs.saturating_add(meta.log_messages.len());
            for log in &meta.log_messages {
                log_bytes = log_bytes.saturating_add(log.len());
            }
        }
        if instructions > limits.max_instructions
            || instructions > u32::MAX as usize
            || logs > limits.max_log_messages
            || logs > u32::MAX as usize
            || log_bytes > limits.max_log_bytes
            || input_bytes > limits.max_input_bytes
            || entries > limits.max_account_entries
        {
            return Err(ProjectError::Capacity);
        }
    }
    Ok(())
}
