use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pancakeswap;

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
        if let Some(swap) = decode_instruction(ix, token_mints) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) = scoped_program_log(log_message, &pancakeswap::PROGRAM_ID.to_vec(), &mut self.is_invoked)? else {
            return None;
        };

        let event = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        let (input_amount, output_amount) = if event.zero_for_one {
            (event.amount_0, event.amount_1)
        } else {
            (event.amount_1, event.amount_0)
        };

        Some(pb::Swap {
            protocol: pb::Protocol::Pancakeswap as i32,
            program_id: pancakeswap::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: pancakeswap::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state.clone(),
            user: instruction.user.clone(),
            input_mint: instruction.input_mint.clone(),
            input_amount,
            output_mint: instruction.output_mint.clone(),
            output_amount,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    user: Vec<u8>,
    pool_state: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
}

struct SwapEvent {
    amount_0: u64,
    amount_1: u64,
    zero_for_one: bool,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_pool(ix)
}

fn decode_instruction(ix: &InstructionView, token_mints: &TokenMintLookup) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &pancakeswap::PROGRAM_ID {
        return None;
    }

    match pancakeswap::instructions::unpack(ix.data()) {
        Ok(pancakeswap::instructions::PancakeSwapInstruction::Swap(_)) => {
            let accounts = pancakeswap::accounts::get_swap_accounts(ix).ok()?;
            let input_token_account = accounts.input_token_account.to_bytes().to_vec();
            let output_token_account = accounts.output_token_account.to_bytes().to_vec();

            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_mint: token_mints.mint_for(&input_token_account)?,
                output_mint: token_mints.mint_for(&output_token_account)?,
            })
        }
        _ => None,
    }
}

fn decode_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    let program_id = ix.program_id().0;
    if program_id != &pancakeswap::PROGRAM_ID {
        return None;
    }

    match pancakeswap::instructions::unpack(ix.data()) {
        Ok(pancakeswap::instructions::PancakeSwapInstruction::Swap(_)) => {
            let accounts = pancakeswap::accounts::get_swap_accounts(ix).ok()?;
            Some(accounts.pool_state.to_bytes().to_vec())
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<SwapEvent> {
    let data = parse_program_data(log_message)?;
    match pancakeswap::events::unpack(data.as_slice()) {
        Ok(pancakeswap::events::PancakeSwapEvent::Swap(event)) => Some(SwapEvent {
            amount_0: event.amount_0,
            amount_1: event.amount_1,
            zero_for_one: event.zero_for_one,
        }),
        _ => None,
    }
}
