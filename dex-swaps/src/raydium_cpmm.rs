use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_data, parse_program_id};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta}};
use substreams_solana_idls::raydium;

pub(crate) fn decode_raydium_cpmm_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let Some(tx_meta) = tx.meta.as_ref() else {
        return Vec::new();
    };

    let instructions = tx.walk_instructions().filter_map(decode_cpmm_instruction).collect::<Vec<_>>();
    let logs = decode_cpmm_logs(tx_meta);

    if instructions.len() != logs.len() {
        return Vec::new();
    }

    instructions
        .into_iter()
        .zip(logs)
        .map(|(instruction, log)| pb::Swap {
            protocol: pb::Protocol::RaydiumCpmm as i32,
            program_id: raydium::cpmm::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::cpmm::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state,
            user: instruction.payer,
            input_mint: log.input_mint.unwrap_or(instruction.input_token_mint),
            input_amount: log.input_amount,
            output_mint: log.output_mint.unwrap_or(instruction.output_token_mint),
            output_amount: log.output_amount,
        })
        .collect()
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

fn decode_cpmm_instruction(ix: InstructionView) -> Option<InstructionSwap> {
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

fn decode_cpmm_logs(tx_meta: &TransactionStatusMeta) -> Vec<LogSwap> {
    let mut logs = Vec::new();
    let mut is_invoked = false;

    for log_message in tx_meta.log_messages.iter() {
        let matches_program = parse_program_id(log_message).map_or(false, |id| id == raydium::cpmm::PROGRAM_ID.to_vec());

        if is_invoke(log_message) && matches_program {
            if let Some(log) = parse_log_data(log_message, parse_invoke_depth(log_message).unwrap_or_default()) {
                logs.push(log);
            }
            is_invoked = true;
        } else if matches_program && (is_success(log_message) || is_failed(log_message)) {
            is_invoked = false;
        } else if is_invoked {
            if let Some(log) = parse_log_data(log_message, 0) {
                logs.push(log);
            }
        }
    }

    logs
}

fn parse_log_data(log_message: &str, _invoke_depth: u32) -> Option<LogSwap> {
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
