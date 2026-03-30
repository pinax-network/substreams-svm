use common::solana::{get_fee_payer, is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_data, parse_program_id};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
use substreams_solana_idls::jupiter;

pub(crate) fn decode_jupiter_v4_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    let tx_meta = match tx.meta.as_ref() {
        Some(meta) => meta,
        None => return Vec::new(),
    };
    let mut instructions = Vec::new();
    let mut is_invoked = false;
    let mut current_stack_height = 0;

    for log_message in &tx_meta.log_messages {
        let is_jupiter_program = parse_program_id(log_message).map_or(false, |id| id == jupiter::v4::PROGRAM_ID.to_vec());
        if is_invoke(log_message) && is_jupiter_program {
            if let Some(height) = parse_invoke_depth(log_message) {
                current_stack_height = height - 1;
                is_invoked = true;
                continue;
            }
        } else if is_jupiter_program && (is_success(log_message) || is_failed(log_message)) {
            is_invoked = false;
            continue;
        }
        if !is_invoked {
            continue;
        }
        let data = match parse_program_data(log_message) {
            Some(data) => data,
            None => continue,
        };
        if let Ok(jupiter::v4::events::JupiterV4Event::Swap(event)) = jupiter::v4::events::unpack(data.as_slice()) {
            instructions.push(pb::Swap {
                program_id: jupiter::v4::PROGRAM_ID.to_vec(),
                protocol: pb::Protocol::JupiterV4 as i32,
                stack_height: current_stack_height,
                amm: event.amm.to_bytes().to_vec(),
                amm_pool: [].to_vec(),
                user: get_fee_payer(&tx).unwrap_or_default(),
                input_mint: event.input_mint.to_bytes().to_vec(),
                input_amount: event.input_amount,
                output_mint: event.output_mint.to_bytes().to_vec(),
                output_amount: event.output_amount,
            });
        }
    }
    instructions
}
