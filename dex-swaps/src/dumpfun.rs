use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana_idls::dumpfun;

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
            scoped_program_log(log_message, &dumpfun::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let data = parse_program_data(log_message)?;
        match dumpfun::events::unpack_event(data.as_slice()) {
            Ok(dumpfun::events::DumpfunEvent::BuyTokenEvent(event)) => Some(pb::Swap {
                protocol: pb::Protocol::Dumpfun as i32,
                program_id: dumpfun::PROGRAM_ID.to_vec(),
                stack_height: 0,
                amm: dumpfun::PROGRAM_ID.to_vec(),
                // Dumpfun is a bonding-curve program: each token has its own
                // curve, so the per-token mint is the stable per-pool
                // identifier surfaced in the event payload (no separate pool
                // PDA exposed). See substreams-svm#210 item 1.
                amm_pool: event.mint.to_bytes().to_vec(),
                user: event.user.to_bytes().to_vec(),
                input_mint: SOL_MINT.to_vec(),
                input_amount: event.sol_in,
                output_mint: event.mint.to_bytes().to_vec(),
                output_amount: event.token_out,
            }),
            Ok(dumpfun::events::DumpfunEvent::SellTokenEvent(event)) => Some(pb::Swap {
                protocol: pb::Protocol::Dumpfun as i32,
                program_id: dumpfun::PROGRAM_ID.to_vec(),
                stack_height: 0,
                amm: dumpfun::PROGRAM_ID.to_vec(),
                amm_pool: event.mint.to_bytes().to_vec(),
                user: event.user.to_bytes().to_vec(),
                input_mint: event.mint.to_bytes().to_vec(),
                input_amount: event.token_in,
                output_mint: SOL_MINT.to_vec(),
                output_amount: event.sol_out,
            }),
            _ => None,
        }
    }
}
