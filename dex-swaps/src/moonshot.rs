use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana_idls::moonshot;

use crate::{
    logs::{scoped_program_log, ProgramLog},
    SOL_MINT,
};

pub(crate) struct State {
    is_invoked: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self { is_invoked: false }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) = scoped_program_log(log_message, &moonshot::PROGRAM_ID.to_vec(), &mut self.is_invoked)? else {
            return None;
        };

        let data = parse_program_data(log_message)?;
        match moonshot::events::unpack_event(data.as_slice()) {
            Ok(moonshot::events::MoonshotEvent::TradeEvent(event)) => {
                let is_buy = matches!(event.trade_type, moonshot::events::TradeType::Buy);
                Some(pb::Swap {
                    protocol: pb::Protocol::Moonshot as i32,
                    program_id: moonshot::PROGRAM_ID.to_vec(),
                    stack_height: 0,
                    amm: moonshot::PROGRAM_ID.to_vec(),
                    amm_pool: event.curve.to_bytes().to_vec(),
                    user: event.sender.to_bytes().to_vec(),
                    input_mint: if is_buy { SOL_MINT.to_vec() } else { event.cost_token.to_bytes().to_vec() },
                    input_amount: if is_buy { event.collateral_amount } else { event.amount },
                    output_mint: if is_buy { event.cost_token.to_bytes().to_vec() } else { SOL_MINT.to_vec() },
                    output_amount: if is_buy { event.amount } else { event.collateral_amount },
                })
            }
            _ => None,
        }
    }
}
