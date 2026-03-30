use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::pumpfun::bonding_curve as pumpfun;

use crate::SOL_MINT;

pub(crate) fn decode_pumpfun_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let mut swaps = Vec::new();
    let mut pending_trade = None;

    for instruction in tx.walk_instructions() {
        let program_id = instruction.program_id().0;
        if program_id != &pumpfun::PROGRAM_ID {
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

        swaps.push(pb::Swap {
            protocol: pb::Protocol::Pumpfun as i32,
            program_id: pumpfun::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height(),
            amm: pumpfun::PROGRAM_ID.to_vec(),
            amm_pool: trade.bonding_curve,
            user: event.user,
            input_mint: if event.is_buy { SOL_MINT.to_vec() } else { event.mint.clone() },
            input_amount: if event.is_buy { event.sol_amount } else { event.token_amount },
            output_mint: if event.is_buy { event.mint } else { SOL_MINT.to_vec() },
            output_amount: if event.is_buy { event.token_amount } else { event.sol_amount },
        });
    }

    swaps
}

struct PendingTrade {
    bonding_curve: Vec<u8>,
}

struct TradeEvent {
    mint: Vec<u8>,
    sol_amount: u64,
    token_amount: u64,
    is_buy: bool,
    user: Vec<u8>,
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

fn decode_trade_event(instruction: &InstructionView) -> Option<TradeEvent> {
    match pumpfun::events::unpack(instruction.data()) {
        Ok(pumpfun::events::PumpFunEvent::TradeV0(event)) => Some(TradeEvent {
            mint: event.mint.to_bytes().to_vec(),
            sol_amount: event.sol_amount,
            token_amount: event.token_amount,
            is_buy: event.is_buy,
            user: event.user.to_bytes().to_vec(),
        }),
        Ok(pumpfun::events::PumpFunEvent::TradeV1(event)) => Some(TradeEvent {
            mint: event.mint.to_bytes().to_vec(),
            sol_amount: event.sol_amount,
            token_amount: event.token_amount,
            is_buy: event.is_buy,
            user: event.user.to_bytes().to_vec(),
        }),
        Ok(pumpfun::events::PumpFunEvent::TradeV2(event)) => Some(TradeEvent {
            mint: event.mint.to_bytes().to_vec(),
            sol_amount: event.sol_amount,
            token_amount: event.token_amount,
            is_buy: event.is_buy,
            user: event.user.to_bytes().to_vec(),
        }),
        Ok(pumpfun::events::PumpFunEvent::TradeV3(event)) => Some(TradeEvent {
            mint: event.mint.to_bytes().to_vec(),
            sol_amount: event.sol_amount,
            token_amount: event.token_amount,
            is_buy: event.is_buy,
            user: event.user.to_bytes().to_vec(),
        }),
        _ => None,
    }
}
