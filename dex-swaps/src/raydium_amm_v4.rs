use common::solana::{is_failed, is_invoke, is_success, parse_program_id, parse_raydium_log};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta}};
use substreams_solana_idls::raydium;

pub(crate) fn decode_raydium_amm_v4_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
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
            let is_pc_to_coin = log.direction == 2;
            let (input_mint, output_mint) = if is_pc_to_coin {
                (instruction.amm_pc_vault.clone(), instruction.amm_coin_vault.clone())
            } else {
                (instruction.amm_coin_vault.clone(), instruction.amm_pc_vault.clone())
            };

            pb::Swap {
                protocol: pb::Protocol::RaydiumAmmV4 as i32,
                program_id: raydium::amm::v4::PROGRAM_ID.to_vec(),
                stack_height: instruction.stack_height,
                amm: raydium::amm::v4::PROGRAM_ID.to_vec(),
                amm_pool: instruction.amm,
                user: instruction.user_source_owner,
                input_mint,
                input_amount: log.amount_in,
                output_mint,
                output_amount: log.amount_out,
            }
        })
        .collect()
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

fn decode_instruction(ix: InstructionView) -> Option<InstructionSwap> {
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

fn decode_logs(tx_meta: &TransactionStatusMeta) -> Vec<LogSwap> {
    let mut logs = Vec::new();
    let mut is_invoked = false;

    for log_message in tx_meta.log_messages.iter() {
        let matches_program =
            parse_program_id(log_message).map_or(false, |id| id == raydium::amm::v4::PROGRAM_ID.to_vec());

        if is_invoke(log_message) && matches_program {
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
