use std::collections::VecDeque;

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

/// Hold pending swaps as a FIFO queue rather than a single `Option` slot. The
/// queue is consumed front-to-back in `walk_instructions` order, which on
/// chain matches the order in which each swap's anchor-CPI `Swap` event
/// fires after its own instruction. The single-slot variant could silently
/// lose the first swap's context if a second swap instruction overwrote
/// `pending` before the first event arrived (e.g. on a future anchor-CPI
/// event variant the adapter doesn't yet match).
pub(crate) fn handle_instruction(pending: &mut VecDeque<PendingSwap>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &dlmm::PROGRAM_ID {
        return None;
    }

    if let Some(swap) = decode_swap_instruction(instruction) {
        pending.push_back(swap);
        return None;
    }

    let event = decode_swap_event(instruction)?;
    let swap = pending.pop_front()?;
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

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    if ix.program_id().0 != &dlmm::PROGRAM_ID {
        return None;
    }
    decode_swap_instruction(ix).map(|s| s.lb_pair)
}

fn decode_swap_instruction(ix: &InstructionView) -> Option<PendingSwap> {
    // Meteora DLMM ships six swap-flavour instructions. Each carries the same
    // (lb_pair, user, token_x_mint, token_y_mint) in different positions of
    // its account list and emits the same anchor CPI `Swap` event with a
    // `swap_for_y` direction flag — handled uniformly by `decode_swap_event`.
    //
    // Pre-v0.5.0 we matched only the original `Swap` instruction, silently
    // dropping ~5/6 of post-launch DLMM volume routed through the variants
    // below.
    match dlmm::instructions::unpack(ix.data()) {
        Ok(dlmm::instructions::MeteoraDlmmInstruction::Swap(_)) => {
            let a = dlmm::accounts::get_swap_accounts(ix).ok()?;
            Some(PendingSwap {
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::Swap2(_)) => {
            let a = dlmm::accounts::get_swap2_accounts(ix).ok()?;
            Some(PendingSwap {
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapExactOut(_)) => {
            let a = dlmm::accounts::get_swap_exact_out_accounts(ix).ok()?;
            Some(PendingSwap {
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapExactOut2(_)) => {
            let a = dlmm::accounts::get_swap_exact_out2_accounts(ix).ok()?;
            Some(PendingSwap {
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapWithPriceImpact(_)) => {
            let a = dlmm::accounts::get_swap_with_price_impact_accounts(ix).ok()?;
            Some(PendingSwap {
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapWithPriceImpact2(_)) => {
            let a = dlmm::accounts::get_swap_with_price_impact2_accounts(ix).ok()?;
            Some(PendingSwap {
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
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
