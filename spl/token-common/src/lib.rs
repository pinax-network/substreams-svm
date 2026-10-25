pub mod accounts;
pub mod balances;
pub mod memo;
pub mod metadata;
pub mod mints;
pub mod permissions;
pub mod transfers;
use common::solana::{get_fee_payer, get_signers};
use proto::pb::solana::spl::token::v1 as pb;
use substreams::errors::Error;
use substreams_solana::block_view::InstructionView;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction};

// Token Program KEG (TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA)
pub const SOLANA_TOKEN_PROGRAM_KEG: [u8; 32] = spl_transfers::TOKEN_PROGRAM_ID;

// Token-2022 Program (TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb)
pub const SOLANA_TOKEN_PROGRAM_ZQB: [u8; 32] = spl_transfers::TOKEN_2022_PROGRAM_ID;

// Memo Program V1 (Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo)
pub const SOLANA_MEMO_PROGRAM_V1: [u8; 32] = [
    5, 74, 83, 80, 248, 93, 200, 130, 214, 20, 165, 86, 114, 120, 138, 41, 109, 223, 30, 171, 171, 208, 166, 6, 120, 136, 73, 50, 244, 238, 246, 160,
];

// Memo Program V2 (MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr)
pub const SOLANA_MEMO_PROGRAM_V2: [u8; 32] = [
    5, 74, 83, 90, 153, 41, 33, 6, 77, 36, 232, 113, 96, 218, 56, 124, 124, 53, 181, 221, 188, 146, 187, 129, 228, 31, 168, 64, 65, 5, 68, 141,
];

pub fn is_spl_memo_program(program_id: &[u8]) -> bool {
    program_id == &SOLANA_MEMO_PROGRAM_V1 || program_id == &SOLANA_MEMO_PROGRAM_V2
}

spl_transfers::block_view_buffa_lenient!(fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

/// Processes the successful transactions of a complete block.
///
/// The block is validated first (`spl_transfers::validate_block_except`), so
/// malformed input is an error rather than a panic or a wrong account. Pass
/// the unfiltered block: positions are original only when no transaction was
/// removed upstream.
///
/// Legacy, v0 and v1 messages are processed. Transactions with a message shape
/// the shared decoder does not cover (an explicit version above v1, a v1
/// message with address table lookups, or inline transaction configuration
/// outside v1) are logged and skipped instead of halting the stream; the other
/// transactions keep their original indexes.
pub fn process_block(block: &Block, is_spl_token_program: fn(&[u8]) -> bool) -> Result<pb::Events, Error> {
    let (view, unsupported) = view_block(block);
    for message in &unsupported {
        substreams::log::info!("skipping {}", message);
    }
    spl_transfers::validate_block_except(&view, &unsupported)?;
    let mut transactions = Vec::new();
    for (index, tx) in block.transactions.iter().enumerate() {
        if !tx.is_successful() || unsupported.iter().any(|message| message.transaction_index == index) {
            continue;
        }
        if let Some(transaction) = process_transaction(tx, index as u32, is_spl_token_program)? {
            transactions.push(transaction);
        }
    }
    Ok(pb::Events { transactions })
}

/// Processes one transaction of a block validated by [`process_block`];
/// `transaction_index` is its original position in the block.
pub fn process_transaction(
    tx: &ConfirmedTransaction,
    transaction_index: u32,
    is_spl_token_program: fn(&[u8]) -> bool,
) -> Result<Option<pb::Transaction>, Error> {
    let Some(tx_meta) = tx.meta.as_option() else {
        return Ok(None);
    };
    let signers = get_signers(tx).unwrap_or_default();

    // Process SPL-Token Balances
    let post_token_balances: Vec<_> = tx_meta
        .post_token_balances
        .iter()
        .filter_map(|balance| balances::get_token_balance(tx, balance, is_spl_token_program))
        .collect();

    let pre_token_balances: Vec<_> = tx_meta
        .pre_token_balances
        .iter()
        .filter_map(|balance| balances::get_token_balance(tx, balance, is_spl_token_program))
        .collect();

    // Walk outer instructions, each followed by its inner instructions, keeping original positions.
    let mut instructions = Vec::new();
    for (index, instruction) in tx.compiled_instructions().enumerate() {
        let instruction_index = index as u32;
        instructions.extend(process_instruction(&instruction, instruction_index, None, is_spl_token_program)?);
        for (inner_index, inner) in instruction.inner_instructions().enumerate() {
            instructions.extend(process_instruction(&inner, instruction_index, Some(inner_index as u32), is_spl_token_program)?);
        }
    }

    if instructions.is_empty() && pre_token_balances.is_empty() && post_token_balances.is_empty() {
        return Ok(None);
    }

    Ok(Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed.unwrap_or_default(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers,
        instructions,
        post_token_balances,
        pre_token_balances,
        transaction_index,
    }))
}

pub fn process_instruction(
    instruction: &InstructionView,
    instruction_index: u32,
    inner_instruction_index: Option<u32>,
    is_spl_token_program: fn(&[u8]) -> bool,
) -> Result<Option<pb::Instruction>, Error> {
    let program_id = instruction.program_id().0;

    // Skip non-SPL-Token & SPL Memo instructions
    if !is_spl_token_program(program_id) && !is_spl_memo_program(program_id) {
        return Ok(None);
    }

    // Try each instruction parser in sequence ordered by frequency
    let transfer = transfers::unpack_transfers(instruction, program_id, is_spl_token_program).map_err(|error| {
        let position = spl_transfers::Position {
            instruction_index,
            inner_instruction_index,
        };
        Error::msg(format!("transaction {} {position}: {error}", instruction.confirmed_transaction().id()))
    })?;
    let parsed_instruction = transfer
        .or_else(|| accounts::unpack_accounts(instruction, program_id, is_spl_token_program))
        .or_else(|| permissions::unpack_permissions(instruction, program_id, is_spl_token_program))
        .or_else(|| mints::unpack_mints(instruction, program_id, is_spl_token_program))
        .or_else(|| metadata::unpack_metadata(instruction, program_id, is_spl_token_program))
        .or_else(|| memo::unpack_memo(instruction, program_id));

    Ok(parsed_instruction.map(|parsed| pb::Instruction {
        program_id: program_id.to_vec(),
        stack_height: instruction.stack_height(),
        is_root: instruction.is_root(),
        instruction_index,
        inner_instruction_index,
        instruction: Some(parsed),
    }))
}
