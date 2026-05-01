use common::solana::{get_fee_payer, parse_program_data};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
use substreams_solana_idls::jupiter;

use crate::logs::{scoped_program_log, ProgramLog};
use crate::routed_pool::Tracker;

pub(crate) struct State {
    is_invoked: bool,
    current_stack_height: u32,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            is_invoked: false,
            current_stack_height: 0,
        }
    }

    pub(crate) fn handle_log(
        &mut self,
        tx: &ConfirmedTransaction,
        log_message: &str,
        routed_pools: &Tracker,
    ) -> Option<pb::Swap> {
        match scoped_program_log(log_message, &jupiter::v4::PROGRAM_ID.to_vec(), &mut self.is_invoked)? {
            ProgramLog::Enter {
                invoke_depth: Some(height),
            } => {
                self.current_stack_height = height - 1;
                return None;
            }
            ProgramLog::Enter { invoke_depth: None } | ProgramLog::Exit => return None,
            ProgramLog::Data(log_message) => {
                let data = parse_program_data(log_message)?;
                if let Ok(jupiter::v4::events::JupiterV4Event::Swap(event)) = jupiter::v4::events::unpack(data.as_slice()) {
                    let amm_program = event.amm.to_bytes();
                    let amm_pool = routed_pools
                        .lookup(&amm_program)
                        .cloned()
                        .unwrap_or_default();
                    return Some(pb::Swap {
                        program_id: jupiter::v4::PROGRAM_ID.to_vec(),
                        protocol: pb::Protocol::JupiterV4 as i32,
                        stack_height: self.current_stack_height,
                        amm: amm_program.to_vec(),
                        amm_pool,
                        user: get_fee_payer(&tx).unwrap_or_default(),
                        input_mint: event.input_mint.to_bytes().to_vec(),
                        input_amount: event.input_amount,
                        output_mint: event.output_mint.to_bytes().to_vec(),
                        output_amount: event.output_amount,
                    });
                }
            }
        }
        None
    }
}
