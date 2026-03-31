use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::darklake;

use crate::logs::{scoped_program_log, ProgramLog};

pub(crate) struct State {
    pending: Vec<bool>,
    next_index: usize,
    is_invoked: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            pending: Vec::new(),
            next_index: 0,
            is_invoked: false,
        }
    }

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView) {
        let program_id = ix.program_id().0;
        if program_id != &darklake::PROGRAM_ID {
            return;
        }

        if let Ok(darklake::instructions::DarklakeInstruction::Swap(event)) = darklake::instructions::unpack(ix.data()) {
            self.pending.push(event.is_swap_x_to_y);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &darklake::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let data = parse_program_data(log_message)?;
        let event = match darklake::events::unpack_event(data.as_slice()) {
            Ok(darklake::events::DarklakeEvent::Swap(event)) => event,
            _ => return None,
        };
        let is_x_to_y = *self.pending.get(self.next_index)?;
        self.next_index += 1;
        let (input_mint, output_mint) = if is_x_to_y {
            (event.token_mint_x.to_bytes().to_vec(), event.token_mint_y.to_bytes().to_vec())
        } else {
            (event.token_mint_y.to_bytes().to_vec(), event.token_mint_x.to_bytes().to_vec())
        };

        Some(pb::Swap {
            protocol: pb::Protocol::Darklake as i32,
            program_id: darklake::PROGRAM_ID.to_vec(),
            stack_height: 0,
            amm: darklake::PROGRAM_ID.to_vec(),
            amm_pool: darklake::PROGRAM_ID.to_vec(),
            user: event.trader.to_bytes().to_vec(),
            input_mint,
            input_amount: event.amount_in,
            output_mint,
            output_amount: event.amount_out,
        })
    }
}
