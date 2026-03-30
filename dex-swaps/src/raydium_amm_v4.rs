use common::solana::parse_raydium_log;
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
            scoped_program_log(log_message, &raydium::amm::v4::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let log = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        let is_pc_to_coin = log.direction == 2;
        let (input_mint, output_mint) = if is_pc_to_coin {
            (instruction.amm_pc_vault.clone(), instruction.amm_coin_vault.clone())
        } else {
            (instruction.amm_coin_vault.clone(), instruction.amm_pc_vault.clone())
        };

        Some(pb::Swap {
            protocol: pb::Protocol::RaydiumAmmV4 as i32,
            program_id: raydium::amm::v4::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::amm::v4::PROGRAM_ID.to_vec(),
            amm_pool: instruction.amm.clone(),
            user: instruction.user_source_owner.clone(),
            input_mint,
            input_amount: log.amount_in,
            output_mint,
            output_amount: log.amount_out,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    amm: Vec<u8>,
    amm_coin_vault: Vec<u8>,
    amm_pc_vault: Vec<u8>,
    user_source_owner: Vec<u8>,
}

struct LogSwap {
    direction: u64,
    amount_in: u64,
    amount_out: u64,
}

fn decode_instruction(ix: &InstructionView) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::amm::v4::PROGRAM_ID {
        return None;
    }

    match raydium::amm::v4::instructions::unpack(ix.data()) {
        Ok(raydium::amm::v4::instructions::RaydiumV4Instruction::SwapBaseIn(_))
        | Ok(raydium::amm::v4::instructions::RaydiumV4Instruction::SwapBaseOut(_)) => {
            let with_target_orders = ix.accounts().len() == 18;
            let offset = if with_target_orders { 1 } else { 0 };
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                amm: ix.accounts()[1].0.to_vec(),
                amm_coin_vault: ix.accounts()[4 + offset].0.to_vec(),
                amm_pc_vault: ix.accounts()[5 + offset].0.to_vec(),
                user_source_owner: ix.accounts()[16 + offset].0.to_vec(),
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_raydium_log(log_message)?;
    match raydium::amm::v4::logs::unpack(data.as_slice()) {
        Ok(raydium::amm::v4::logs::RaydiumV4Log::SwapBaseIn(event)) => Some(LogSwap {
            direction: event.direction,
            amount_in: event.amount_in,
            amount_out: event.out_amount,
        }),
        Ok(raydium::amm::v4::logs::RaydiumV4Log::SwapBaseOut(event)) => Some(LogSwap {
            direction: event.direction,
            amount_in: event.deduct_in,
            amount_out: event.amount_out,
        }),
        _ => None,
    }
}
