use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::meteora::dllm;

pub(crate) fn decode_meteora_dllm_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let mut swaps = Vec::new();
    let mut pending_swap = None;

    for instruction in tx.walk_instructions() {
        let program_id = instruction.program_id().0;
        if program_id != &dllm::PROGRAM_ID {
            continue;
        }

        if let Some(swap) = decode_swap_instruction(&instruction) {
            pending_swap = Some(swap);
            continue;
        }

        let Some(event) = decode_swap_event(&instruction) else {
            continue;
        };

        let Some(swap) = pending_swap.take() else {
            continue;
        };

        let (input_mint, output_mint) = if event.swap_for_y {
            (swap.token_x_mint.clone(), swap.token_y_mint.clone())
        } else {
            (swap.token_y_mint.clone(), swap.token_x_mint.clone())
        };

        swaps.push(pb::Swap {
            protocol: pb::Protocol::MeteoraDllm as i32,
            program_id: dllm::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height(),
            amm: dllm::PROGRAM_ID.to_vec(),
            amm_pool: swap.lb_pair,
            user: swap.user,
            input_mint,
            input_amount: event.amount_in,
            output_mint,
            output_amount: event.amount_out,
        });
    }

    swaps
}

struct PendingSwap {
    user: Vec<u8>,
    lb_pair: Vec<u8>,
    token_x_mint: Vec<u8>,
    token_y_mint: Vec<u8>,
}

struct SwapEvent {
    amount_in: u64,
    amount_out: u64,
    swap_for_y: bool,
}

fn decode_swap_instruction(ix: &InstructionView) -> Option<PendingSwap> {
    match dllm::instructions::unpack(ix.data()) {
        Ok(dllm::instructions::MeteoraDllmInstruction::Swap(_)) => {
            let accounts = dllm::accounts::get_swap_accounts(ix).ok()?;
            Some(PendingSwap {
                user: accounts.user.to_bytes().to_vec(),
                lb_pair: accounts.lb_pair.to_bytes().to_vec(),
                token_x_mint: accounts.token_x_mint.to_bytes().to_vec(),
                token_y_mint: accounts.token_y_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn decode_swap_event(ix: &InstructionView) -> Option<SwapEvent> {
    match dllm::anchor_cpi_event::unpack(ix.data()) {
        Ok(dllm::anchor_cpi_event::MeteoraDllmAnchorCpiEvent::Swap(event)) => Some(SwapEvent {
            amount_in: event.amount_in,
            amount_out: event.amount_out,
            swap_for_y: event.swap_for_y,
        }),
        _ => None,
    }
}
