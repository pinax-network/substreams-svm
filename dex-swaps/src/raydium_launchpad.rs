use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::raydium;

pub(crate) fn decode_raydium_launchpad_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let mut swaps = Vec::new();
    let mut pending_trade = None;

    for instruction in tx.walk_instructions() {
        let program_id = instruction.program_id().0;
        if program_id != &raydium::launchpad::PROGRAM_ID {
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

        let is_buy = event.is_buy;
        let (input_mint, output_mint) = if is_buy {
            (trade.quote_token_mint.clone(), trade.base_token_mint.clone())
        } else {
            (trade.base_token_mint.clone(), trade.quote_token_mint.clone())
        };

        swaps.push(pb::Swap {
            protocol: pb::Protocol::RaydiumLaunchpad as i32,
            program_id: raydium::launchpad::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height(),
            amm: raydium::launchpad::PROGRAM_ID.to_vec(),
            amm_pool: trade.pool_state,
            user: trade.payer,
            input_mint,
            input_amount: event.amount_in,
            output_mint,
            output_amount: event.amount_out,
        });
    }

    swaps
}

struct PendingTrade {
    payer: Vec<u8>,
    pool_state: Vec<u8>,
    base_token_mint: Vec<u8>,
    quote_token_mint: Vec<u8>,
}

struct TradeEvent {
    amount_in: u64,
    amount_out: u64,
    is_buy: bool,
}

fn decode_trade_instruction(ix: &InstructionView) -> Option<PendingTrade> {
    let trade = match raydium::launchpad::instructions::unpack(ix.data()) {
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::BuyExactIn(evt)) => {
            let accounts = raydium::launchpad::accounts::get_buy_exact_in_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec(), evt.amount_in))
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::BuyExactOut(evt)) => {
            let accounts = raydium::launchpad::accounts::get_buy_exact_out_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec(), evt.amount_out))
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::SellExactIn(evt)) => {
            let accounts = raydium::launchpad::accounts::get_sell_exact_in_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec(), evt.amount_in))
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::SellExactOut(evt)) => {
            let accounts = raydium::launchpad::accounts::get_sell_exact_out_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec(), evt.amount_out))
        }
        _ => None,
    }?;

    Some(PendingTrade {
        payer: trade.0,
        pool_state: trade.1,
        base_token_mint: trade.2,
        quote_token_mint: trade.3,
    })
}

fn decode_trade_event(ix: &InstructionView) -> Option<TradeEvent> {
    let event = match raydium::launchpad::anchor_cpi_event::unpack(ix.data()) {
        Ok(raydium::launchpad::anchor_cpi_event::RaydiumLaunchpadAnchorCpiEvent::TradeEventV1(event)) => {
            Some((event.amount_in, event.amount_out, matches!(
                event.trade_direction,
                raydium::launchpad::anchor_cpi_event::TradeDirection::Buy
            )))
        }
        Ok(raydium::launchpad::anchor_cpi_event::RaydiumLaunchpadAnchorCpiEvent::TradeEventV2(event)) => {
            Some((event.amount_in, event.amount_out, matches!(
                event.trade_direction,
                raydium::launchpad::anchor_cpi_event::TradeDirection::Buy
            )))
        }
        _ => None,
    }?;

    Some(TradeEvent {
        amount_in: event.0,
        amount_out: event.1,
        is_buy: event.2,
    })
}
