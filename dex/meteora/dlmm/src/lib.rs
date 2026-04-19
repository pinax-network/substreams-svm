use common::solana::{get_fee_payer, get_signers};
use proto::pb::meteora::dlmm::v1 as pb;
use substreams::errors::Error;
use substreams_solana::{
    block_view::InstructionView,
    pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction},
};
use substreams_solana_idls::meteora::dlmm;

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: block.transactions_owned().filter_map(process_transaction).collect(),
    })
}

fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;

    let instructions: Vec<pb::Instruction> = tx.walk_instructions().filter_map(|iv| process_instruction(&iv)).collect();

    if instructions.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(&tx).unwrap_or_default(),
        signers: get_signers(&tx).unwrap_or_default(),
        instructions,
    })
}

fn process_instruction(ix: &InstructionView) -> Option<pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &dlmm::PROGRAM_ID {
        return None;
    }

    // 1) Try to decode Anchor "event CPI" first and EARLY-RETURN if it matches.
    if let Ok(dlmm::anchor_cpi_event::MeteoraDlmmAnchorCpiEvent::Swap(event)) = dlmm::anchor_cpi_event::unpack(ix.data()) {
        return Some(pb::Instruction {
            program_id: program_id.to_vec(),
            stack_height: ix.stack_height(),
            instruction: Some(pb::instruction::Instruction::SwapEvent(pb::SwapEvent {
                lb_pair: event.lb_pair.to_bytes().to_vec(),
                from: event.from.to_bytes().to_vec(),
                start_bin_id: event.start_bin_id,
                end_bin_id: event.end_bin_id,
                amount_in: event.amount_in,
                amount_out: event.amount_out,
                swap_for_y: event.swap_for_y,
                fee: event.fee,
                protocol_fee: event.protocol_fee,
                fee_bps: event.fee_bps.to_string(),
                host_fee: event.host_fee,
            })),
        });
    }

    match dlmm::instructions::unpack(ix.data()) {
        Ok(dlmm::instructions::MeteoraDlmmInstruction::Swap(evt)) => {
            let accounts = dlmm::accounts::get_swap_accounts(ix).ok()?;
            Some(pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(pb::instruction::Instruction::SwapInstruction(pb::SwapInstruction {
                    accounts: Some(pb::SwapAccounts {
                        lb_pair: accounts.lb_pair.to_bytes().to_vec(),
                        bin_array_bitmap_extension: accounts.bin_array_bitmap_extension.map(|a| a.to_bytes().to_vec()).unwrap_or_default(),
                        reserve_x: accounts.reserve_x.to_bytes().to_vec(),
                        reserve_y: accounts.reserve_y.to_bytes().to_vec(),
                        user_token_in: accounts.user_token_in.to_bytes().to_vec(),
                        user_token_out: accounts.user_token_out.to_bytes().to_vec(),
                        token_x_mint: accounts.token_x_mint.to_bytes().to_vec(),
                        token_y_mint: accounts.token_y_mint.to_bytes().to_vec(),
                        oracle: accounts.oracle.to_bytes().to_vec(),
                        host_fee_in: accounts.host_fee_in.map(|a| a.to_bytes().to_vec()).unwrap_or_default(),
                        user: accounts.user.to_bytes().to_vec(),
                        token_x_program: accounts.token_x_program.to_bytes().to_vec(),
                        token_y_program: accounts.token_y_program.to_bytes().to_vec(),
                        event_authority: accounts.event_authority.to_bytes().to_vec(),
                        program: accounts.program.to_bytes().to_vec(),
                    }),
                    amount_in: evt.amount_in,
                    min_amount_out: evt.min_amount_out,
                })),
            })
        }
        _ => None,
    }
}
