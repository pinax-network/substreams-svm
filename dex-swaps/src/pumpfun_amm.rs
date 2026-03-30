use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::pumpfun::amm as pumpfun_amm;

pub(crate) fn decode_pumpfun_amm_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let mut swaps = Vec::new();
    let mut pending_trade = None;

    for instruction in tx.walk_instructions() {
        let program_id = instruction.program_id().0;
        if program_id != &pumpfun_amm::PROGRAM_ID {
            continue;
        }

        if let Some(trade) = decode_trade_instruction(&instruction) {
            pending_trade = Some(trade);
            continue;
        }

        let Some(event) = decode_trade_event(&instruction) else {
            continue;
        };

        let Some(trade) = pending_trade.take() else {
            continue;
        };

        let (input_amount, output_amount) = if event.is_buy {
            (event.quote_amount, event.base_amount)
        } else {
            (event.base_amount, event.quote_amount)
        };

        swaps.push(pb::Swap {
            protocol: pb::Protocol::PumpfunAmm as i32,
            program_id: pumpfun_amm::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height(),
            amm: pumpfun_amm::PROGRAM_ID.to_vec(),
            amm_pool: trade.pool,
            user: event.user,
            input_mint: if event.is_buy { trade.quote_mint.clone() } else { trade.base_mint.clone() },
            input_amount,
            output_mint: if event.is_buy { trade.base_mint } else { trade.quote_mint },
            output_amount,
        });
    }

    swaps
}

struct PendingTrade {
    pool: Vec<u8>,
    base_mint: Vec<u8>,
    quote_mint: Vec<u8>,
}

struct TradeEvent {
    is_buy: bool,
    user: Vec<u8>,
    base_amount: u64,
    quote_amount: u64,
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    match pumpfun_amm::instructions::unpack(instruction.data()) {
        Ok(pumpfun_amm::instructions::PumpFunAmmInstruction::Buy(_))
        | Ok(pumpfun_amm::instructions::PumpFunAmmInstruction::Sell(_)) => Some(PendingTrade {
            pool: instruction.accounts().get(0)?.0.to_vec(),
            base_mint: instruction.accounts().get(4)?.0.to_vec(),
            quote_mint: instruction.accounts().get(5)?.0.to_vec(),
        }),
        _ => None,
    }
}

fn decode_trade_event(instruction: &InstructionView) -> Option<TradeEvent> {
    match pumpfun_amm::events::unpack(instruction.data()) {
        Ok(pumpfun_amm::events::PumpFunAmmEvent::BuyEventV1(event)) => Some(TradeEvent {
            is_buy: true,
            user: event.user.to_bytes().to_vec(),
            base_amount: event.base_amount_out,
            quote_amount: event.quote_amount_in,
        }),
        Ok(pumpfun_amm::events::PumpFunAmmEvent::BuyEventV2(event)) => Some(TradeEvent {
            is_buy: true,
            user: event.user.to_bytes().to_vec(),
            base_amount: event.base_amount_out,
            quote_amount: event.quote_amount_in,
        }),
        Ok(pumpfun_amm::events::PumpFunAmmEvent::SellEventV1(event)) => Some(TradeEvent {
            is_buy: false,
            user: event.user.to_bytes().to_vec(),
            base_amount: event.base_amount_in,
            quote_amount: event.quote_amount_out,
        }),
        Ok(pumpfun_amm::events::PumpFunAmmEvent::SellEventV2(event)) => Some(TradeEvent {
            is_buy: false,
            user: event.user.to_bytes().to_vec(),
            base_amount: event.base_amount_in,
            quote_amount: event.quote_amount_out,
        }),
        _ => None,
    }
}
