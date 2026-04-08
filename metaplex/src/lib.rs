mod metadata;

use common::solana::{get_fee_payer, get_signers};
use proto::pb::solana::metaplex::v1 as pb;
use substreams::errors::Error;
use substreams_solana::block_view::InstructionView;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction};
use substreams_solana_idls::metaplex::token_metadata;

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: block.transactions_owned().filter_map(process_transaction).collect(),
    })
}

fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let signers = get_signers(&tx).unwrap_or_default();

    let instructions: Vec<_> = tx.walk_instructions().filter_map(|iview| process_instruction(&iview)).collect();

    if instructions.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(&tx).unwrap_or_default(),
        signers,
        instructions,
    })
}

fn process_instruction(instruction: &InstructionView) -> Option<pb::Instruction> {
    let program_id = instruction.program_id().0;

    if program_id != &token_metadata::PROGRAM_ID {
        return None;
    }

    metadata::unpack_metadata(instruction).map(|parsed| pb::Instruction {
        program_id: program_id.to_vec(),
        stack_height: instruction.stack_height(),
        is_root: instruction.is_root(),
        instruction: Some(parsed),
    })
}
