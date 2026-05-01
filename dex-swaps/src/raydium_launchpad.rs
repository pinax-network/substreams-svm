use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::raydium;

pub(crate) struct PendingTrade {
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

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &raydium::launchpad::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    let event = decode_trade_event(instruction)?;
    let trade = pending_trade.take()?;
    let (input_mint, output_mint) = if event.is_buy {
        (trade.quote_token_mint.clone(), trade.base_token_mint.clone())
    } else {
        (trade.base_token_mint.clone(), trade.quote_token_mint.clone())
    };

    Some(pb::Swap {
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
    })
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    if ix.program_id().0 != &raydium::launchpad::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(ix).map(|t| t.pool_state)
}

fn decode_trade_instruction(ix: &InstructionView) -> Option<PendingTrade> {
    let trade = match raydium::launchpad::instructions::unpack(ix.data()) {
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::BuyExactIn(_evt)) => {
            let accounts = raydium::launchpad::accounts::get_buy_exact_in_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec()))
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::BuyExactOut(_evt)) => {
            let accounts = raydium::launchpad::accounts::get_buy_exact_out_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec()))
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::SellExactIn(_evt)) => {
            let accounts = raydium::launchpad::accounts::get_sell_exact_in_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec()))
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::SellExactOut(_evt)) => {
            let accounts = raydium::launchpad::accounts::get_sell_exact_out_accounts(ix).ok()?;
            Some((accounts.payer.to_bytes().to_vec(), accounts.pool_state.to_bytes().to_vec(), accounts.base_token_mint.to_bytes().to_vec(), accounts.quote_token_mint.to_bytes().to_vec()))
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
