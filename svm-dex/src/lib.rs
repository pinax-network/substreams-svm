use common::db::{common_key_v2, set_clock};
use proto::pb::dex::swaps::v1 as pb;
use substreams::{errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_solana::base58;

#[substreams::handlers::map]
pub fn db_out(mut clock: Clock, dex_swaps_events: pb::Events) -> Result<DatabaseChanges, Error> {
    let mut tables = substreams_database_change::tables::Tables::new();

    for (transaction_index, transaction) in dex_swaps_events.transactions.iter().enumerate() {
        for (swap_index, swap) in transaction.swaps.iter().enumerate() {
            let key = common_key_v2(&clock, transaction_index, swap_index);
            let row = tables
                .create_row("swaps", key)
                .set("signature", base58::encode(&transaction.signature))
                .set("fee_payer", base58::encode(&transaction.fee_payer))
                .set(
                    "signers_raw",
                    transaction.signers.iter().map(base58::encode).collect::<Vec<_>>().join(","),
                )
                .set("fee", transaction.fee)
                .set("compute_units_consumed", transaction.compute_units_consumed)
                .set("amm", base58::encode(&swap.amm))
                .set("amm_pool", base58::encode(&swap.amm_pool))
                .set("user", base58::encode(&swap.user))
                .set("input_mint", base58::encode(&swap.input_mint))
                .set("input_amount", swap.input_amount)
                .set("output_mint", base58::encode(&swap.output_mint))
                .set("output_amount", swap.output_amount);

            set_clock(&clock, row);
        }
    }

    if tables.all_row_count() > 0 {
        set_clock(&clock, tables.create_row("blocks", [("block_num", clock.number.to_string())]));
    }

    Ok(tables.to_database_changes())
}
