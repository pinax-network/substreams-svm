use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpswap;
use substreams_solana_idls::pumpswap::events::TradeDirection;

pub(crate) struct PendingTrade {
    pool: Vec<u8>,
    base_mint: Vec<u8>,
    quote_mint: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpswap::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in `substreams_solana_idls::pumpswap`
    // — read the comment there for the rationale (pump.fun has appended fields
    // to BuyEvent/SellEvent multiple times since launch and we don't want each
    // sink chasing the IDL).
    let event = pumpswap::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    let is_buy = matches!(event.direction, TradeDirection::Buy);
    let (input_amount, output_amount) = if is_buy {
        (event.quote_amount, event.base_amount)
    } else {
        (event.base_amount, event.quote_amount)
    };

    Some(pb::Swap {
        protocol: pb::Protocol::PumpfunAmm as i32,
        program_id: pumpswap::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: pumpswap::PROGRAM_ID.to_vec(),
        amm_pool: trade.pool,
        user: event.user.to_vec(),
        input_mint: if is_buy { trade.quote_mint.clone() } else { trade.base_mint.clone() },
        input_amount,
        output_mint: if is_buy { trade.base_mint } else { trade.quote_mint },
        output_amount,
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpswap::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(instruction).map(|t| t.pool)
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    match pumpswap::instructions::unpack(instruction.data()) {
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_)) => Some(PendingTrade {
            pool: instruction.accounts().get(0)?.0.to_vec(),
            base_mint: instruction.accounts().get(4)?.0.to_vec(),
            quote_mint: instruction.accounts().get(5)?.0.to_vec(),
        }),
        _ => None,
    }
}
