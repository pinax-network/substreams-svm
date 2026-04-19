use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::meteora::dlmm;

pub(crate) struct PendingSwap {
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

pub(crate) fn handle_instruction(pending_swap: &mut Option<PendingSwap>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &dlmm::PROGRAM_ID {
        return None;
    }

    if let Some(swap) = decode_swap_instruction(instruction) {
        *pending_swap = Some(swap);
        return None;
    }

    let event = decode_swap_event(instruction)?;
    let swap = pending_swap.take()?;
    let (input_mint, output_mint) = if event.swap_for_y {
        (swap.token_x_mint.clone(), swap.token_y_mint.clone())
    } else {
        (swap.token_y_mint.clone(), swap.token_x_mint.clone())
    };

    Some(pb::Swap {
        protocol: pb::Protocol::MeteoraDlmm as i32,
        program_id: dlmm::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: dlmm::PROGRAM_ID.to_vec(),
        amm_pool: swap.lb_pair,
        user: swap.user,
        input_mint,
        input_amount: event.amount_in,
        output_mint,
        output_amount: event.amount_out,
    })
}

fn decode_swap_instruction(ix: &InstructionView) -> Option<PendingSwap> {
    match dlmm::instructions::unpack(ix.data()) {
        Ok(dlmm::instructions::MeteoraDlmmInstruction::Swap(_)) => {
            let accounts = dlmm::accounts::get_swap_accounts(ix).ok()?;
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
    match dlmm::anchor_cpi_event::unpack(ix.data()) {
        Ok(dlmm::anchor_cpi_event::MeteoraDlmmAnchorCpiEvent::Swap(event)) => Some(SwapEvent {
            amount_in: event.amount_in,
            amount_out: event.amount_out,
            swap_for_y: event.swap_for_y,
        }),
        _ => None,
    }
}
