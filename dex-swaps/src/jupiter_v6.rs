use common::solana::get_fee_payer;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::jupiter;

pub(crate) fn decode_jupiter_v6_transaction(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    tx.walk_instructions().filter_map(|instruction| decode_jupiter_v6_swap(tx, &instruction)).collect()
}

fn decode_jupiter_v6_swap(tx: &ConfirmedTransaction, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &jupiter::v6::PROGRAM_ID {
        return None;
    }

    match jupiter::v6::events::unpack(instruction.data()) {
        Ok(jupiter::v6::events::JupiterV6Event::Swap(event)) => Some(pb::Swap {
            program_id: jupiter::v6::PROGRAM_ID.to_vec(),
            protocol: pb::Protocol::JupiterV6 as i32,
            stack_height: instruction.stack_height(),
            amm: event.amm.to_bytes().to_vec(),
            amm_pool: [].to_vec(),
            user: get_fee_payer(tx).unwrap_or_default(),
            input_mint: event.input_mint.to_bytes().to_vec(),
            input_amount: event.input_amount,
            output_mint: event.output_mint.to_bytes().to_vec(),
            output_amount: event.output_amount,
        }),
        _ => None,
    }
}
