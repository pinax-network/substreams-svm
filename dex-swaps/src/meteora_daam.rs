use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::meteora::daam;

pub(crate) struct PendingSwap {
    stack_height: u32,
    user: Vec<u8>,
    pool: Vec<u8>,
    token_a_mint: Vec<u8>,
    token_b_mint: Vec<u8>,
}

struct SwapEvent {
    amount_in: u64,
    output_amount: u64,
    trade_direction: u8,
}

pub(crate) fn handle_instruction(pending_swap: &mut Option<PendingSwap>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &daam::PROGRAM_ID {
        return None;
    }

    if let Some(swap) = decode_swap_instruction(instruction) {
        *pending_swap = Some(swap);
        return None;
    }

    let event = decode_swap_event(instruction)?;
    let swap = pending_swap.take()?;
    let (input_mint, output_mint) = if event.trade_direction == 0 {
        (swap.token_a_mint.clone(), swap.token_b_mint.clone())
    } else {
        (swap.token_b_mint.clone(), swap.token_a_mint.clone())
    };

    Some(pb::Swap {
        protocol: pb::Protocol::MeteoraDaam as i32,
        program_id: daam::PROGRAM_ID.to_vec(),
        stack_height: swap.stack_height,
        amm: daam::PROGRAM_ID.to_vec(),
        amm_pool: swap.pool,
        user: swap.user,
        input_mint,
        input_amount: event.amount_in,
        output_mint,
        output_amount: event.output_amount,
    })
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_swap_instruction(ix).map(|s| s.pool)
}

fn decode_swap_instruction(ix: &InstructionView) -> Option<PendingSwap> {
    let program_id = ix.program_id().0;
    if program_id != &daam::PROGRAM_ID {
        return None;
    }

    match daam::instructions::unpack(ix.data()) {
        Ok(daam::instructions::MeteoraDammInstruction::Swap(_)) | Ok(daam::instructions::MeteoraDammInstruction::Swap2(_)) => {
            // `swap` and `swap2` share the same 14-account list per the
            // canonical cp_amm IDL; only the args struct (dynamic-fee /
            // referral params) differs. Same anchor-CPI `EvtSwap` emit.
            let accounts = daam::accounts::get_swap_accounts(ix).ok()?;
            Some(PendingSwap {
                stack_height: ix.stack_height(),
                user: accounts.payer.to_bytes().to_vec(),
                pool: accounts.pool.to_bytes().to_vec(),
                token_a_mint: accounts.token_a_mint.to_bytes().to_vec(),
                token_b_mint: accounts.token_b_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn decode_swap_event(ix: &InstructionView) -> Option<SwapEvent> {
    match daam::anchor_cpi_event::unpack(ix.data()) {
        Ok(daam::anchor_cpi_event::MeteoraDammAnchorCpiEvent::EvtSwap(event)) => Some(SwapEvent {
            amount_in: event.actual_amount_in,
            output_amount: event.swap_result.output_amount,
            trade_direction: event.trade_direction,
        }),
        // Since the cp-amm upgrade on 2025-12-26 both `swap` and `swap2`
        // emit `EvtSwap2` instead of `EvtSwap`. `included_transfer_fee_amount_in`
        // mirrors what the user paid (pre-pool fees) and is the analogue of
        // the legacy `actual_amount_in`.
        Ok(daam::anchor_cpi_event::MeteoraDammAnchorCpiEvent::EvtSwap2(event)) => Some(SwapEvent {
            amount_in: event.included_transfer_fee_amount_in,
            output_amount: event.swap_result.output_amount,
            trade_direction: event.trade_direction,
        }),
        _ => None,
    }
}
