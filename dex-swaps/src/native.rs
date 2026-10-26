//! Native complete-block API with source evidence separate from legacy rows.
use crate::{pb, process_transaction_with_provenance};
pub(crate) use common::invocation::instruction_positions;
pub use common::invocation::{InstructionPosition, InvocationOutcome};
use std::collections::BTreeMap;
use std::fmt;
use substreams_solana::block_view::InstructionView;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction, TransactionStatusMeta};
use substreams_solana_idls::{jupiter, okx};

spl_transfers::block_view_buffa!(fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

/// Location of the evidence that emitted a decoded row, not a guessed location
/// of an earlier swap invocation. Self-CPI events carry their event instruction's
/// position. Log rows keep the original transaction log index; the invocation
/// that wrote the line is reported separately in [`ProjectedSwap::invocation`]
/// only when the complete invocation trace proves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    Instruction { position: InstructionPosition, event_index: u32 },
    Log { log_index: u32, event_index: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapKind {
    PoolExecution,
    /// Jupiter and OKX summaries can duplicate separately decoded venue rows.
    AggregatorSummary,
}

/// One decoded row plus the evidence the native API can prove about it.
///
/// `invocation`, `invocation_outcome` and `aggregator_route` describe the
/// writer of the row's evidence. Only `verified_context` proves that the row's
/// pool, user, mints and amounts belong to one swap instruction. A row is
/// identified by its `provenance`: instruction position and event index, or
/// log index and event index.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedSwap {
    pub swap: pb::Swap,
    pub kind: SwapKind,
    pub provenance: Provenance,
    /// Outcome of `invocation` and every enclosing invocation, from the exact
    /// trace; Unknown when `invocation` is None. For a log row this is the
    /// outcome of the invocation that wrote the evidence line, and says
    /// nothing about which instruction the adapter paired with that line.
    pub invocation_outcome: InvocationOutcome,
    /// The swap instruction whose accounts, together with this row's own
    /// evidence, independently rebuild every row field, after proven success
    /// of that invocation and every ancestor. None means unverified, even
    /// when the invocation succeeded. It proves which instruction the fields
    /// came from, not what they mean: a verified row keeps the adapter's own
    /// field semantics.
    /// - Meteora DLMM instruction rows: the swap that made the event self-CPI,
    ///   which is the parent of `invocation`.
    /// - Orca Whirlpool, Raydium AMM v4/CLMM/CPMM and Meteora AMM log rows:
    ///   `invocation` itself, when its swap instruction and the evidence line
    ///   rebuild the row.
    /// - OKX summaries: `invocation` itself, when that `SwapV3` instruction and
    ///   only the lines it wrote itself complete this summary at this line.
    ///   `output_amount` stays the last `SwapEvent`'s `amount_out`: the
    ///   route's final swap output when the route is one chain whose last hop
    ///   uses one dex, otherwise one leg of the last hop or last chain; in
    ///   both cases not adjusted for OKX commission or platform fees. An
    ///   unverified OKX summary may carry another `SwapV3`'s user, mints and
    ///   `input_amount`, even one in another route.
    /// - Every other protocol: always None.
    pub verified_context: Option<InstructionPosition>,
    /// The invocation that wrote this row's evidence, proved by an exact
    /// alignment of the instruction walk with the runtime invocation logs.
    /// Instruction rows: the event instruction. Log rows: the innermost
    /// invocation executing when the evidence line was written, reported only
    /// when its program is the row's `program_id`. For a log row this names
    /// the writer of the line, not the instruction the legacy adapter paired
    /// with it (see `verified_context`). None when another program wrote the
    /// line, or the trace is missing, truncated, ambiguous or inconsistent.
    pub invocation: Option<InstructionPosition>,
    /// The outermost Jupiter v4, Jupiter v6 or OKX invocation enclosing (or
    /// equal to) `invocation`: the route the evidence writer ran in. Nested
    /// aggregators share the outermost route, so one route can hold summaries
    /// from several aggregators that describe the same hop. None has two
    /// meanings: with `invocation` Some, the row is proved to be outside every
    /// route; with `invocation` None, route membership is unknown, not
    /// "unrouted".
    ///
    /// To count a routed trade once, group only rows whose `invocation` is
    /// Some by `(transaction_index, aggregator_route)`, and per route count
    /// exactly one of: its venue legs (`PoolExecution` rows), or its route
    /// summaries. The route summaries are the Jupiter v4 summaries whose
    /// `invocation` equals `aggregator_route` and the OKX summaries whose
    /// `verified_context` equals it (written, and for OKX also rebuilt, by the
    /// route invocation itself) or, when there are none, the route's Jupiter
    /// v6 summaries, so a nested aggregator's summaries are never counted on
    /// top of the route's own. An unverified OKX summary is never a route
    /// summary: it may carry another route's `SwapV3` fields. A route without
    /// route summaries is counted by its legs. Summaries and legs do not
    /// correspond one to one (a route may have hops without a decoded leg, or
    /// legs without a decoded summary), so choose per route. The crate README
    /// details the rule and its one limit.
    pub aggregator_route: Option<InstructionPosition>,
    /// Rank of this row's evidence in the transaction's proved invocation
    /// trace, from 0 across instruction and log rows: an instruction row at its
    /// event instruction's `invoke` line, a log row at its own line, then by
    /// event index. It orders evidence lines only. It does not attribute the
    /// row (an unattributed log row is ranked too), and it is not a stable
    /// identity: a decoder change that adds or drops a row renumbers the rest.
    /// Set for every row of a transaction whose trace is proved, else for none.
    pub execution_ordinal: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedTransaction {
    /// Original index in the complete block, including votes and failures.
    pub transaction_index: u32,
    pub signature: Vec<u8>,
    pub fee_payer: Vec<u8>,
    pub signers: Vec<Vec<u8>>,
    pub fee: u64,
    pub compute_units_consumed: u64,
    /// Existing adapter output order (instruction rows, then log rows), kept
    /// identical to the legacy wrapper. Sort by
    /// [`ProjectedSwap::execution_ordinal`] for the order of the evidence in
    /// the proved trace. Ordinals are set for every row exactly when the
    /// transaction's trace is proved, so `swaps[0].execution_ordinal.is_some()`
    /// says whether it was.
    pub swaps: Vec<ProjectedSwap>,
}

impl ProjectedTransaction {
    pub(crate) fn into_legacy(self) -> pb::Transaction {
        pb::Transaction {
            signature: self.signature,
            fee_payer: self.fee_payer,
            signers: self.signers,
            fee: self.fee,
            compute_units_consumed: self.compute_units_consumed,
            swaps: self.swaps.into_iter().map(|row| row.swap).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clock {
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: String,
    pub previous_blockhash: String,
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
    pub transaction_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    pub clock: Clock,
    pub transactions: Vec<ProjectedTransaction>,
}

impl Projection {
    /// Remove added provenance without changing any legacy row field or order.
    pub fn into_legacy(self) -> pb::Events {
        pb::Events {
            transactions: self.transactions.into_iter().map(ProjectedTransaction::into_legacy).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_transactions: usize,
    pub max_instructions: usize,
    pub max_log_messages: usize,
    pub max_log_bytes: usize,
    pub max_swaps: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_transactions: 100_000,
            max_instructions: 100_000,
            max_log_messages: 1_000_000,
            max_log_bytes: 64 * 1024 * 1024,
            max_swaps: 100_000,
        }
    }
}

#[derive(Debug)]
pub enum ProjectError {
    Capacity,
    InvalidBlock(spl_transfers::ProjectError),
    UnsupportedMessage(spl_transfers::UnsupportedMessage),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => f.write_str("native swap projection capacity exceeded"),
            Self::InvalidBlock(_) => f.write_str("invalid complete Solana block"),
            Self::UnsupportedMessage(error) => write!(f, "unsupported Solana message shape: {error}"),
        }
    }
}
impl std::error::Error for ProjectError {}

/// Validate and project one complete block without executing Substreams. This
/// exposes existing adapter behavior; it does not repair missing variants,
/// pool/event association defects or prove the success of every inner call.
pub fn project(block: &Block) -> Result<Projection, ProjectError> {
    project_with_limits(block, Limits::default())
}

pub fn project_with_limits(block: &Block, limits: Limits) -> Result<Projection, ProjectError> {
    if block.transactions.len() > limits.max_transactions || block.transactions.len() > u32::MAX as usize {
        return Err(ProjectError::Capacity);
    }
    let (mut instructions, mut logs, mut log_bytes) = (0usize, 0usize, 0usize);
    for tx in &block.transactions {
        if let Some(message) = tx.transaction.as_option().and_then(|tx| tx.message.as_option()) {
            instructions = instructions.saturating_add(message.instructions.len());
        }
        if let Some(meta) = tx.meta.as_option() {
            for group in &meta.inner_instructions {
                instructions = instructions.saturating_add(group.instructions.len());
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
        {
            return Err(ProjectError::Capacity);
        }
    }
    let view = view_block(block).map_err(ProjectError::UnsupportedMessage)?;
    spl_transfers::validate_block(&view).map_err(ProjectError::InvalidBlock)?;
    let mut transactions = Vec::new();
    let mut swaps = 0usize;
    for (index, tx) in block.transactions.iter().enumerate() {
        if tx.meta.as_option().is_some_and(|meta| meta.err.is_set()) {
            continue;
        }
        if let Some(mut transaction) = process_transaction_with_provenance(tx, limits.max_swaps.saturating_sub(swaps))? {
            transaction.transaction_index = index as u32;
            if let Some(trace) = crate::native_outcome::prove(tx) {
                annotate(tx, &trace, &mut transaction.swaps);
            }
            swaps += transaction.swaps.len();
            transactions.push(transaction);
        }
    }
    Ok(Projection {
        clock: Clock {
            slot: block.slot,
            parent_slot: block.parent_slot,
            blockhash: block.blockhash.clone(),
            previous_blockhash: block.previous_blockhash.clone(),
            block_time: block.block_time.as_option().map(|time| time.timestamp),
            block_height: block.block_height.as_option().map(|height| height.block_height),
            transaction_count: block.transactions.len() as u32,
        },
        transactions,
    })
}

/// Add proved invocation evidence to rows already extracted from `tx`. Row
/// order and every legacy field stay unchanged.
fn annotate(tx: &ConfirmedTransaction, trace: &crate::native_outcome::Trace, rows: &mut [ProjectedSwap]) {
    let Some(meta) = tx.meta.as_option() else { return };
    let instructions: BTreeMap<_, _> = instruction_positions(tx).into_iter().zip(tx.walk_instructions()).collect();
    let routes = aggregator_routes(trace, &instructions);
    let verifier = LogRowVerifier::new(tx, meta, trace, &instructions, rows);
    let mut order_keys = Vec::with_capacity(rows.len());
    for row in rows.iter_mut() {
        let order_key = match row.provenance {
            Provenance::Instruction { position, event_index } => {
                let evidence = trace.evidence.get(&position);
                if let Some(evidence) = evidence {
                    row.invocation_outcome = evidence.outcome;
                    if evidence.invoke_log_index.is_some() {
                        row.invocation = Some(position);
                    }
                    if evidence.outcome == InvocationOutcome::Succeeded {
                        if let Some(parent) = evidence.parent {
                            if let (Some(event), Some(context)) = (instructions.get(&position), instructions.get(&parent)) {
                                if crate::meteora_dlmm::matches_context(context, event, &row.swap) {
                                    row.verified_context = Some(parent);
                                }
                            }
                        }
                    }
                }
                evidence.and_then(|evidence| evidence.invoke_log_index).map(|line| (line, event_index))
            }
            Provenance::Log { log_index, event_index } => {
                let emitter = trace.emitters.get(log_index as usize).copied().flatten();
                if let Some((emitter, view, evidence)) = emitter.and_then(|emitter| Some((emitter, instructions.get(&emitter)?, trace.evidence.get(&emitter)?)))
                {
                    if view.program_id().0.as_slice() == row.swap.program_id.as_slice() {
                        row.invocation = Some(emitter);
                        row.invocation_outcome = evidence.outcome;
                        if evidence.outcome == InvocationOutcome::Succeeded && verifier.rebuilds(view, log_index, &row.swap) {
                            row.verified_context = Some(emitter);
                        }
                    }
                }
                Some((log_index, event_index))
            }
        };
        row.aggregator_route = row.invocation.and_then(|invocation| routes.get(&invocation).copied());
        order_keys.push(order_key);
    }
    if let Some(ordinals) = order_keys.into_iter().collect::<Option<Vec<_>>>().and_then(|keys| execution_ordinals(&keys)) {
        for (row, ordinal) in rows.iter_mut().zip(ordinals) {
            row.execution_ordinal = Some(ordinal);
        }
    }
}

/// Dense ranks of `keys` (trace line, event index), or None when two rows
/// share a key. Keys are distinct by construction: an instruction row's key is
/// its event instruction's own `invoke` line, a log row's key is a program
/// output line (`Program data:`/`Program log:`), never an `invoke` line, and
/// rows at one line differ by event index. A tie would make the order the
/// adapters' rather than the trace's, so it withholds every ordinal.
pub(crate) fn execution_ordinals(keys: &[(u32, u32)]) -> Option<Vec<u32>> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_unstable_by_key(|&index| keys[index]);
    let distinct = order.windows(2).all(|pair| keys[pair[0]] != keys[pair[1]]);
    debug_assert!(distinct, "execution order keys must be distinct");
    if !distinct {
        return None;
    }
    let mut ordinals = vec![0; keys.len()];
    for (ordinal, index) in order.into_iter().enumerate() {
        ordinals[index] = ordinal as u32;
    }
    Some(ordinals)
}

/// Rebuilds a log row from the proved invocation that wrote its evidence line.
struct LogRowVerifier<'a> {
    logs: &'a [String],
    token_mints: crate::token_mints::TokenMintLookup,
    /// OKX summaries rebuilt from each `SwapV3` invocation's own lines, by the
    /// line completing them. Empty unless the transaction has an OKX log row.
    okx_summaries: BTreeMap<u32, pb::Swap>,
}

impl<'a> LogRowVerifier<'a> {
    fn new(
        tx: &ConfirmedTransaction,
        meta: &'a TransactionStatusMeta,
        trace: &crate::native_outcome::Trace,
        instructions: &BTreeMap<InstructionPosition, InstructionView>,
        rows: &[ProjectedSwap],
    ) -> Self {
        let okx_rows = rows
            .iter()
            .any(|row| matches!(row.provenance, Provenance::Log { .. }) && row.swap.protocol == pb::Protocol::OkxDex);
        Self {
            logs: &meta.log_messages,
            token_mints: crate::token_mints::TokenMintLookup::new(tx, meta),
            okx_summaries: if okx_rows {
                crate::okx::summaries_by_writer(instructions, &meta.log_messages, &trace.emitters)
            } else {
                BTreeMap::new()
            },
        }
    }

    /// True when `emitter`, the proved writer of line `log_index`, rebuilds
    /// exactly `row` from its own instruction and evidence.
    fn rebuilds(&self, emitter: &InstructionView, log_index: u32, row: &pb::Swap) -> bool {
        let line = &self.logs[log_index as usize];
        match row.protocol.as_known() {
            Some(pb::Protocol::MeteoraAmm) => crate::meteora_amm::verify_log_row(emitter, line, &self.token_mints, row),
            Some(pb::Protocol::OkxDex) => self.okx_summaries.get(&log_index) == Some(row),
            Some(pb::Protocol::OrcaWhirlpool) => crate::orca_whirlpool::verify_log_row(emitter, line, &self.token_mints, row),
            Some(pb::Protocol::RaydiumAmmV4) => crate::raydium_amm_v4::verify_log_row(emitter, line, &self.token_mints, row),
            Some(pb::Protocol::RaydiumClmm) => crate::raydium_clmm::verify_log_row(emitter, line, &self.token_mints, row),
            Some(pb::Protocol::RaydiumCpmm) => crate::raydium_cpmm::verify_log_row(emitter, line, row),
            _ => false,
        }
    }
}

/// The outermost Jupiter v4, Jupiter v6 or OKX invocation enclosing (or equal
/// to) each traced invocation. One forward pass: positions iterate in walk
/// order, where every parent precedes its children.
fn aggregator_routes(
    trace: &crate::native_outcome::Trace,
    instructions: &BTreeMap<InstructionPosition, InstructionView>,
) -> BTreeMap<InstructionPosition, InstructionPosition> {
    let mut routes = BTreeMap::new();
    for (position, evidence) in &trace.evidence {
        let inherited = evidence.parent.and_then(|parent| routes.get(&parent).copied());
        let own = instructions.get(position).is_some_and(|instruction| {
            let program = instruction.program_id().0;
            program == &jupiter::v4::PROGRAM_ID || program == &jupiter::v6::PROGRAM_ID || program == &okx::v2::PROGRAM_ID
        });
        if let Some(route) = inherited.or(own.then_some(*position)) {
            routes.insert(*position, route);
        }
    }
    routes
}

pub(crate) struct Collector {
    rows: Vec<ProjectedSwap>,
    current: Provenance,
    max_swaps: usize,
    overflow: bool,
}

impl Collector {
    pub fn new(max_swaps: usize) -> Self {
        Self {
            rows: Vec::new(),
            current: Provenance::Log { log_index: 0, event_index: 0 },
            max_swaps,
            overflow: false,
        }
    }
    pub fn at_instruction(&mut self, position: InstructionPosition) {
        self.current = Provenance::Instruction { position, event_index: 0 };
    }
    pub fn at_log(&mut self, log_index: u32) {
        self.current = Provenance::Log { log_index, event_index: 0 };
    }
    pub fn push(&mut self, swap: pb::Swap) {
        if self.rows.len() >= self.max_swaps {
            self.overflow = true;
            return;
        }
        let kind = if matches!(
            swap.protocol.as_known(),
            Some(pb::Protocol::JupiterV4 | pb::Protocol::JupiterV6 | pb::Protocol::OkxDex)
        ) {
            SwapKind::AggregatorSummary
        } else {
            SwapKind::PoolExecution
        };
        self.rows.push(ProjectedSwap {
            swap,
            kind,
            provenance: self.current,
            invocation_outcome: InvocationOutcome::Unknown,
            verified_context: None,
            invocation: None,
            aggregator_route: None,
            execution_ordinal: None,
        });
        match &mut self.current {
            Provenance::Instruction { event_index, .. } | Provenance::Log { event_index, .. } => *event_index += 1,
        }
    }
    pub fn finish(self) -> Result<Vec<ProjectedSwap>, ProjectError> {
        if self.overflow {
            Err(ProjectError::Capacity)
        } else {
            Ok(self.rows)
        }
    }
}
