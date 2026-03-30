use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_data, parse_program_id};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta}};
use substreams_solana_idls::orca;

pub(crate) fn decode_orca_whirlpool_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let Some(tx_meta) = tx.meta.as_ref() else {
        return Vec::new();
    };

    let instructions = tx.walk_instructions().filter_map(decode_instruction).collect::<Vec<_>>();
    let logs = decode_logs(tx_meta);

    if instructions.len() != logs.len() {
        return Vec::new();
    }

    let mut swaps = Vec::new();
    for (instruction, event) in instructions.into_iter().zip(logs) {
        let (input_mint, output_mint) = if instruction.a_to_b {
            (instruction.mint_a.clone(), instruction.mint_b.clone())
        } else {
            (instruction.mint_b.clone(), instruction.mint_a.clone())
        };

        swaps.push(pb::Swap {
            protocol: pb::Protocol::OrcaWhirlpool as i32,
            program_id: orca::whirlpool::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: orca::whirlpool::PROGRAM_ID.to_vec(),
            amm_pool: instruction.whirlpool,
            user: instruction.user,
            input_mint,
            input_amount: event.input_amount,
            output_mint,
            output_amount: event.output_amount,
        });
    }

    swaps
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

fn decode_instruction(ix: InstructionView) -> Option<InstructionSwap> {
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

fn decode_logs(tx_meta: &TransactionStatusMeta) -> Vec<LogSwap> {
    let mut logs = Vec::new();
    let mut is_invoked = false;

    for log_message in tx_meta.log_messages.iter() {
        let matches_program =
            parse_program_id(log_message).map_or(false, |id| id == orca::whirlpool::PROGRAM_ID.to_vec());

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
    match orca::whirlpool::events::parse_event(data.as_slice()) {
        Ok(orca::whirlpool::events::WhirlpoolEvent::Traded(event)) => Some(LogSwap {
            input_amount: event.input_amount,
            output_amount: event.output_amount,
        }),
        _ => None,
    }
}
