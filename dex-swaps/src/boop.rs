use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana_idls::boop;

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
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &boop::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let data = parse_program_data(log_message)?;
        match boop::events::unpack_event(data.as_slice()) {
            Ok(boop::events::BoopEvent::TokenBoughtEvent(event)) => Some(pb::Swap {
                protocol: pb::Protocol::Boop as i32,
                program_id: boop::PROGRAM_ID.to_vec(),
                stack_height: 0,
                amm: boop::PROGRAM_ID.to_vec(),
                amm_pool: boop::PROGRAM_ID.to_vec(),
                user: event.buyer.to_bytes().to_vec(),
                input_mint: SOL_MINT.to_vec(),
                input_amount: event.amount_in,
                output_mint: event.mint.to_bytes().to_vec(),
                output_amount: event.amount_out,
            }),
            Ok(boop::events::BoopEvent::TokenSoldEvent(event)) => Some(pb::Swap {
                protocol: pb::Protocol::Boop as i32,
                program_id: boop::PROGRAM_ID.to_vec(),
                stack_height: 0,
                amm: boop::PROGRAM_ID.to_vec(),
                amm_pool: boop::PROGRAM_ID.to_vec(),
                user: event.seller.to_bytes().to_vec(),
                input_mint: event.mint.to_bytes().to_vec(),
                input_amount: event.amount_in,
                output_mint: SOL_MINT.to_vec(),
                output_amount: event.amount_out,
            }),
            _ => None,
        }
    }
}
