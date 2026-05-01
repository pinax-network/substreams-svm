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
        if let Some(swap) = decode_instruction(ix) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &raydium::clmm::v3::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let log = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        let (input_amount, output_amount) = if log.zero_for_one {
            (log.amount_0, log.amount_1)
        } else {
            (log.amount_1, log.amount_0)
        };

        Some(pb::Swap {
            protocol: pb::Protocol::RaydiumClmm as i32,
            program_id: raydium::clmm::v3::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::clmm::v3::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state.clone(),
            user: instruction.payer.clone(),
            input_mint: instruction.input_mint.clone(),
            input_amount,
            output_mint: instruction.output_mint.clone(),
            output_amount,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    payer: Vec<u8>,
    pool_state: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
}

struct LogSwap {
    amount_0: u64,
    amount_1: u64,
    zero_for_one: bool,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_instruction(ix).map(|s| s.pool_state)
}

fn decode_instruction(ix: &InstructionView) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::clmm::v3::PROGRAM_ID {
        return None;
    }

    match raydium::clmm::v3::instructions::unpack(ix.data()) {
        Ok(raydium::clmm::v3::instructions::RaydiumClmmInstruction::Swap(event)) => {
            let accounts = raydium::clmm::v3::accounts::get_swap_accounts(&ix).ok()?;
            let (input_mint, output_mint) = if event.is_base_input {
                (accounts.input_vault.to_bytes().to_vec(), accounts.output_vault.to_bytes().to_vec())
            } else {
                (accounts.output_vault.to_bytes().to_vec(), accounts.input_vault.to_bytes().to_vec())
            };
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_mint,
                output_mint,
            })
        }
        Ok(raydium::clmm::v3::instructions::RaydiumClmmInstruction::SwapV2(_event)) => {
            let accounts = raydium::clmm::v3::accounts::get_swap_v2_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_mint: accounts.input_vault_mint.to_bytes().to_vec(),
                output_mint: accounts.output_vault_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_program_data(log_message)?;
    match raydium::clmm::v3::events::unpack(data.as_slice()) {
        Ok(raydium::clmm::v3::events::RaydiumClmmEvent::SwapEvent(event)) => Some(LogSwap {
            amount_0: event.amount_0,
            amount_1: event.amount_1,
            zero_for_one: event.zero_for_one,
        }),
        _ => None,
    }
}
