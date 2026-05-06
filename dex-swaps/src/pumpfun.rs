use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpfun::bonding_curve as pumpfun;

use crate::SOL_MINT;

pub(crate) struct PendingTrade {
    bonding_curve: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpfun::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in
    // `substreams_solana_idls::pumpfun::bonding_curve` — the on-chain
    // TradeEvent has grown past the IDL's V0..V3 fixed lengths, so the strict
    // decoder rejects every current event. The minimal helper reads the
    // stable leading layout at fixed offsets and ignores trailing fields.
    let event = pumpfun::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    let mint = event.mint.to_vec();
    Some(pb::Swap {
        protocol: pb::Protocol::Pumpfun as i32,
        program_id: pumpfun::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: pumpfun::PROGRAM_ID.to_vec(),
        amm_pool: trade.bonding_curve,
        user: event.user.to_vec(),
        input_mint: if event.is_buy { SOL_MINT.to_vec() } else { mint.clone() },
        input_amount: if event.is_buy { event.sol_amount } else { event.token_amount },
        output_mint: if event.is_buy { mint } else { SOL_MINT.to_vec() },
        output_amount: if event.is_buy { event.token_amount } else { event.sol_amount },
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpfun::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(instruction).map(|t| t.bonding_curve)
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    match pumpfun::instructions::unpack(instruction.data()) {
        Ok(pumpfun::instructions::PumpFunInstruction::Buy(_))
        | Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)) => Some(PendingTrade {
            bonding_curve: instruction.accounts().get(3)?.0.to_vec(),
        }),
        _ => None,
    }
}
