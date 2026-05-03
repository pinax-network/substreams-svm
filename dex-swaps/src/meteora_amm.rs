use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::meteora::amm;

use crate::{
    logs::{scoped_program_log, ProgramLog},
    token_mints::TokenMintLookup,
};

pub(crate) struct State {
    pending: Vec<InstructionSwap>,
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

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView, token_mints: &TokenMintLookup) {
        if let Some(swap) = decode_swap_instruction(ix, token_mints) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) = scoped_program_log(log_message, &amm::PROGRAM_ID.to_vec(), &mut self.is_invoked)? else {
            return None;
        };

        let event = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        Some(pb::Swap {
            protocol: pb::Protocol::MeteoraAmm as i32,
            program_id: amm::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: amm::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool.clone(),
            user: instruction.user.clone(),
            input_mint: instruction.input_mint.clone(),
            input_amount: event.in_amount,
            output_mint: instruction.output_mint.clone(),
            output_amount: event.out_amount,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    pool: Vec<u8>,
    user: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
}

struct SwapEvent {
    in_amount: u64,
    out_amount: u64,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    if ix.program_id().0 != &amm::PROGRAM_ID {
        return None;
    }

    match amm::instructions::unpack(ix.data()) {
        Ok(amm::instructions::AmmInstruction::Swap(_)) => {
            let accounts = amm::accounts::get_swap_accounts(ix).ok()?;
            Some(accounts.pool.to_bytes().to_vec())
        }
        _ => None,
    }
}

fn decode_swap_instruction(ix: &InstructionView, token_mints: &TokenMintLookup) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &amm::PROGRAM_ID {
        return None;
    }

    match amm::instructions::unpack(ix.data()) {
        Ok(amm::instructions::AmmInstruction::Swap(_)) => {
            let accounts = amm::accounts::get_swap_accounts(ix).ok()?;
            let source_token = accounts.user_source_token.to_bytes().to_vec();
            let destination_token = accounts.user_destination_token.to_bytes().to_vec();

            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                pool: accounts.pool.to_bytes().to_vec(),
                user: accounts.user.to_bytes().to_vec(),
                input_mint: token_mints.mint_for(&source_token)?,
                output_mint: token_mints.mint_for(&destination_token)?,
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<SwapEvent> {
    let data = parse_program_data(log_message)?;
    match amm::events::parse_event(data.as_slice()) {
        Ok(amm::events::AmmEvent::Swap(event)) => Some(SwapEvent {
            in_amount: event.in_amount,
            out_amount: event.out_amount,
        }),
        _ => None,
    }
}
