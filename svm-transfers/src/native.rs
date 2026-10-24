//! Legacy database rows from the same native projections used by typed consumers.
//! No instruction extraction or protobuf decoding is repeated here.
use common::solana::{get_fee_payer, get_signers};
use native_token::Projection as SystemProjection;
use proto::pb::solana::{native::token::v1 as system, spl::token::v1 as spl};
use spl_transfers::{Coverage, Operation, OperationKind, Projection, TokenProgram};
use substreams::{buffa::Message, errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_solana::base58;

/// Format complete `Coverage::ALL` SPL and system projections of the same block.
///
/// Inputs must come directly from the shared projectors, without filtering or
/// reordering. The caller bounds protobuf bytes/allocations and projector limits
/// before calling. This adapter additionally bounds the combined operation count
/// and encoded output. It preserves the wrapper's table allowlist, order, exact
/// integers, missing values and dense system-event indexes. CPI rows remain
/// instruction observations, not evidence that a caught inner call persisted.
pub fn changes(
    clock: &Clock,
    spl: &Projection,
    system: &SystemProjection<'_>,
    max_operations: usize,
    max_output_bytes: usize,
) -> Result<DatabaseChanges, Error> {
    let block = system.block;
    let time = clock.timestamp.as_option().ok_or_else(|| Error::msg("missing transfer clock"))?;
    if spl.clock.coverage != Coverage::ALL
        || spl.clock.slot != block.slot
        || spl.clock.parent_slot != block.parent_slot
        || base58::encode(spl.clock.blockhash) != block.blockhash
        || base58::encode(spl.clock.previous_blockhash) != block.previous_blockhash
        || spl.clock.transaction_count as usize != block.transactions.len()
        || spl.clock.block_time != block.block_time.as_option().map(|time| time.timestamp)
        || clock.number != block.slot
        || clock.id != block.blockhash
        || Some(time.seconds) != spl.clock.block_time
        || time.nanos != 0
    {
        return Err(Error::msg("mismatched or incomplete transfer projections"));
    }
    if max_operations == 0
        || max_operations > 100_000
        || max_output_bytes == 0
        || max_output_bytes > 64 * 1024 * 1024
        || spl.operations.len().saturating_add(system.operations.len()) > max_operations
    {
        return Err(Error::msg("transfer formatting capacity exceeded"));
    }
    let (spl_events, system_events) = events(spl, system)?;
    let input_bytes = spl_events.try_encoded_len()?.saturating_add(system_events.try_encoded_len()?);
    if input_bytes as usize > max_output_bytes {
        return Err(Error::msg("transfer formatting input capacity exceeded"));
    }
    let changes = crate::changes(clock, &spl_events, &system_events)?;
    if changes.try_encoded_len()? as usize > max_output_bytes {
        return Err(Error::msg("transfer formatting output capacity exceeded"));
    }
    Ok(changes)
}

fn events(spl: &Projection, system: &SystemProjection<'_>) -> Result<(spl::Events, system::Events), Error> {
    let mut spl_events = spl::Events::default();
    let mut system_events = system::Events::default();
    let mut spl_ops = spl.operations.iter().peekable();
    let mut system_ops = system.operations.iter().peekable();
    for (index, transaction) in system.block.transactions.iter().enumerate() {
        if !transaction.is_successful() {
            continue;
        }
        let meta = transaction.meta.as_option().ok_or_else(|| Error::msg("missing transaction metadata"))?;
        let mut instructions = Vec::new();
        while spl_ops.peek().is_some_and(|op| op.transaction_index as usize == index) {
            let op = spl_ops.next().unwrap();
            if transaction.transaction.signatures.first().map(Vec::as_slice) != Some(op.signature.as_slice()) {
                return Err(Error::msg("SPL projection transaction mismatch"));
            }
            instructions.push(spl_instruction(op));
        }
        if !instructions.is_empty() {
            spl_events.transactions.push(spl::Transaction {
                transaction_index: index as u32,
                signature: transaction.hash().to_vec(),
                fee_payer: get_fee_payer(transaction).unwrap_or_default(),
                signers: get_signers(transaction).unwrap_or_default(),
                fee: meta.fee,
                compute_units_consumed: meta.compute_units_consumed.unwrap_or_default(),
                instructions,
                ..Default::default()
            });
        }
        let mut instructions = Vec::new();
        while system_ops.peek().is_some_and(|op| op.transaction_index as usize == index) {
            let op = system_ops.next().unwrap();
            if !std::ptr::eq(op.transaction, transaction) {
                return Err(Error::msg("system projection transaction mismatch"));
            }
            instructions.push(op.instruction.clone());
        }
        // map_events retains balance-only transactions. They consume a dense
        // transaction index in the legacy formatter even though they emit no
        // transfer rows. Preserve that slot without copying all balances.
        let account_count = transaction.resolved_accounts().len();
        let has_balances = account_count > 0 && (meta.pre_balances.len() == account_count || meta.post_balances.len() == account_count);
        if !instructions.is_empty() || has_balances {
            system_events.transactions.push(system::Transaction {
                signature: transaction.hash().to_vec(),
                fee_payer: get_fee_payer(transaction).unwrap_or_default(),
                signers: get_signers(transaction).unwrap_or_default(),
                fee: meta.fee,
                compute_units_consumed: meta.compute_units_consumed.unwrap_or_default(),
                instructions,
                ..Default::default()
            });
        }
    }
    if spl_ops.next().is_some() || system_ops.next().is_some() {
        return Err(Error::msg("transfer projection is out of order or refers to a missing transaction"));
    }
    Ok((spl_events, system_events))
}

fn spl_instruction(op: &Operation) -> spl::Instruction {
    let transfer = spl::Transfer {
        authority: op.authority.to_vec(),
        multisig_authority: op.extra_accounts.iter().map(|account| account.to_vec()).collect(),
        source: op.source.map(|key| key.to_vec()).unwrap_or_default(),
        destination: op.destination.map(|key| key.to_vec()).unwrap_or_default(),
        amount: op.amount,
        mint: op.mint.map(|key| key.to_vec()),
        decimals: op.decimals.map(u32::from),
    };
    spl::Instruction {
        program_id: match op.program {
            TokenProgram::Token => spl_transfers::TOKEN_PROGRAM_ID,
            TokenProgram::Token2022 => spl_transfers::TOKEN_2022_PROGRAM_ID,
        }
        .to_vec(),
        is_root: op.inner_instruction_index.is_none(),
        stack_height: op.stack_height.unwrap_or_default(),
        instruction_index: op.instruction_index,
        inner_instruction_index: op.inner_instruction_index,
        instruction: Some(match op.kind {
            OperationKind::Transfer | OperationKind::TransferChecked => spl::instruction::Instruction::Transfer(transfer.into()),
            OperationKind::MintTo | OperationKind::MintToChecked => spl::instruction::Instruction::Mint(transfer.into()),
            OperationKind::Burn | OperationKind::BurnChecked => spl::instruction::Instruction::Burn(transfer.into()),
        }),
    }
}

#[cfg(test)]
mod tests;
