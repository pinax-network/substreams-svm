use proto::pb::{dex::swaps::v1 as pb, jupiter::v1 as jupiter_pb};
use substreams::errors::Error;

const AMM_JUPITER_V6: &[u8] = b"jupiter-v6";

#[substreams::handlers::map]
fn map_events(jupiter_v6_events: jupiter_pb::Events) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: jupiter_v6_events
            .transactions
            .into_iter()
            .filter_map(map_jupiter_transaction)
            .collect(),
    })
}

fn map_jupiter_transaction(transaction: jupiter_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .into_iter()
        .filter_map(|instruction| match instruction.instruction {
            Some(jupiter_pb::instruction::Instruction::SwapEvent(event)) => Some(pb::Swap {
                amm: AMM_JUPITER_V6.to_vec(),
                amm_pool: event.amm,
                user: transaction.fee_payer.clone(),
                input_mint: event.input_mint,
                input_amount: event.input_amount,
                output_mint: event.output_mint,
                output_amount: event.output_amount,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}
