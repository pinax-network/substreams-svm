use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_data, parse_program_id};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta}};
use substreams_solana_idls::raydium;

pub(crate) fn decode_raydium_clmm_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let Some(tx_meta) = tx.meta.as_ref() else {
        return Vec::new();
    };

    let instructions = tx.walk_instructions().filter_map(decode_instruction).collect::<Vec<_>>();
    let logs = decode_logs(tx_meta);

    if instructions.len() != logs.len() {
        return Vec::new();
    }

    instructions
        .into_iter()
        .zip(logs)
        .map(|(instruction, log)| {
            let (input_amount, output_amount) = if log.zero_for_one {
                (log.amount_0, log.amount_1)
            } else {
                (log.amount_1, log.amount_0)
            };

            pb::Swap {
                protocol: pb::Protocol::RaydiumClmm as i32,
                program_id: raydium::clmm::v3::PROGRAM_ID.to_vec(),
                stack_height: instruction.stack_height,
                amm: raydium::clmm::v3::PROGRAM_ID.to_vec(),
                amm_pool: instruction.pool_state,
                user: instruction.payer,
                input_mint: instruction.input_mint,
                input_amount,
                output_mint: instruction.output_mint,
                output_amount,
            }
        })
        .collect()
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

fn decode_instruction(ix: InstructionView) -> Option<InstructionSwap> {
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

fn decode_logs(tx_meta: &TransactionStatusMeta) -> Vec<LogSwap> {
    let mut logs = Vec::new();
    let mut is_invoked = false;

    for log_message in tx_meta.log_messages.iter() {
        let matches_program =
            parse_program_id(log_message).map_or(false, |id| id == raydium::clmm::v3::PROGRAM_ID.to_vec());

        if is_invoke(log_message) && matches_program {
            let _ = parse_invoke_depth(log_message);
            if let Some(log) = parse_log_data(log_message) {
                logs.push(log);
            }
            is_invoked = true;
        } else if matches_program && (is_success(log_message) || is_failed(log_message)) {
            is_invoked = false;
        } else if is_invoked {
            if let Some(log) = parse_log_data(log_message) {
                logs.push(log);
            }
        }
    }

    logs
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
