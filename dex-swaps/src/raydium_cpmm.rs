use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::raydium;

use crate::logs::{scoped_program_log, ProgramLog};

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

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView) {
        if let Some(swap) = decode_cpmm_instruction(ix) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &raydium::cpmm::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let log = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        Some(pb::Swap {
            protocol: pb::Protocol::RaydiumCpmm as i32,
            program_id: raydium::cpmm::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::cpmm::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state.clone(),
            user: instruction.payer.clone(),
            input_mint: log.input_mint.unwrap_or_else(|| instruction.input_token_mint.clone()),
            input_amount: log.input_amount,
            output_mint: log.output_mint.unwrap_or_else(|| instruction.output_token_mint.clone()),
            output_amount: log.output_amount,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    payer: Vec<u8>,
    pool_state: Vec<u8>,
    input_token_mint: Vec<u8>,
    output_token_mint: Vec<u8>,
}

struct LogSwap {
    input_amount: u64,
    output_amount: u64,
    input_mint: Option<Vec<u8>>,
    output_mint: Option<Vec<u8>>,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_cpmm_instruction(ix).map(|s| s.pool_state)
}

fn decode_cpmm_instruction(ix: &InstructionView) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::cpmm::PROGRAM_ID {
        return None;
    }

    match raydium::cpmm::instructions::unpack(ix.data()) {
        Ok(raydium::cpmm::instructions::RaydiumCpmmInstruction::SwapBaseInput(_)) => {
            let accounts = raydium::cpmm::accounts::get_swap_base_input_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_token_mint: accounts.input_token_mint.to_bytes().to_vec(),
                output_token_mint: accounts.output_token_mint.to_bytes().to_vec(),
            })
        }
        Ok(raydium::cpmm::instructions::RaydiumCpmmInstruction::SwapBaseOutput(_)) => {
            let accounts = raydium::cpmm::accounts::get_swap_base_output_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_token_mint: accounts.input_token_mint.to_bytes().to_vec(),
                output_token_mint: accounts.output_token_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_program_data(log_message)?;
    match raydium::cpmm::events::unpack(data.as_slice()) {
        Ok(raydium::cpmm::events::RaydiumCpmmEvent::SwapEventV1(event)) => Some(LogSwap {
            input_amount: event.input_amount,
            output_amount: event.output_amount,
            input_mint: None,
            output_mint: None,
        }),
        Ok(raydium::cpmm::events::RaydiumCpmmEvent::SwapEventV2(event)) => Some(LogSwap {
            input_amount: event.input_amount,
            output_amount: event.output_amount,
            input_mint: Some(event.input_mint.to_bytes().to_vec()),
            output_mint: Some(event.output_mint.to_bytes().to_vec()),
        }),
        _ => None,
    }
}
