use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::orca;

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
            scoped_program_log(log_message, &orca::whirlpool::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let event = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        let (input_mint, output_mint) = if instruction.a_to_b {
            (instruction.mint_a.clone(), instruction.mint_b.clone())
        } else {
            (instruction.mint_b.clone(), instruction.mint_a.clone())
        };

        Some(pb::Swap {
            protocol: pb::Protocol::OrcaWhirlpool as i32,
            program_id: orca::whirlpool::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: orca::whirlpool::PROGRAM_ID.to_vec(),
            amm_pool: instruction.whirlpool.clone(),
            user: instruction.user.clone(),
            input_mint,
            input_amount: event.input_amount,
            output_mint,
            output_amount: event.output_amount,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    user: Vec<u8>,
    whirlpool: Vec<u8>,
    mint_a: Vec<u8>,
    mint_b: Vec<u8>,
    a_to_b: bool,
}

struct LogSwap {
    input_amount: u64,
    output_amount: u64,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_instruction(ix).map(|s| s.whirlpool)
}

fn decode_instruction(ix: &InstructionView) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &orca::whirlpool::PROGRAM_ID {
        return None;
    }

    match orca::whirlpool::instructions::unpack(ix.data()) {
        Ok(orca::whirlpool::instructions::WhirlpoolInstruction::Swap(event)) => {
            let accounts = orca::whirlpool::accounts::get_swap_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.token_authority.to_bytes().to_vec(),
                whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                mint_a: accounts.token_vault_a.to_bytes().to_vec(),
                mint_b: accounts.token_vault_b.to_bytes().to_vec(),
                a_to_b: event.a_to_b,
            })
        }
        Ok(orca::whirlpool::instructions::WhirlpoolInstruction::SwapV2(event)) => {
            let accounts = orca::whirlpool::accounts::get_swap_v2_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.token_authority.to_bytes().to_vec(),
                whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                mint_a: accounts.token_mint_a.to_bytes().to_vec(),
                mint_b: accounts.token_mint_b.to_bytes().to_vec(),
                a_to_b: event.a_to_b,
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_program_data(log_message)?;
    match orca::whirlpool::events::parse_event(data.as_slice()) {
        Ok(orca::whirlpool::events::WhirlpoolEvent::Traded(event)) => Some(LogSwap {
            input_amount: event.input_amount,
            output_amount: event.output_amount,
        }),
        _ => None,
    }
}
