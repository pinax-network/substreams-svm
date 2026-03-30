use common::db::{common_key_v2, set_clock};
use proto::pb::dex::swaps::v1 as pb;
use substreams::{errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_solana::base58;

fn protocol_slug(protocol: i32) -> &'static str {
    match pb::Protocol::try_from(protocol).unwrap_or(pb::Protocol::Unspecified) {
        pb::Protocol::Unspecified => "unspecified",
        pb::Protocol::Boop => "boop",
        pb::Protocol::Darklake => "darklake",
        pb::Protocol::Dumpfun => "dumpfun",
        pb::Protocol::JupiterV4 => "jupiter_v4",
        pb::Protocol::JupiterV6 => "jupiter_v6",
        pb::Protocol::MeteoraDaam => "meteora_daam",
        pb::Protocol::MeteoraDllm => "meteora_dllm",
        pb::Protocol::OrcaWhirlpool => "orca_whirlpool",
        pb::Protocol::Pumpfun => "pumpfun",
        pb::Protocol::PumpfunAmm => "pumpfun_amm",
        pb::Protocol::RaydiumAmmV4 => "raydium_amm_v4",
        pb::Protocol::RaydiumClmm => "raydium_clmm",
        pb::Protocol::RaydiumCpmm => "raydium_cpmm",
        pb::Protocol::RaydiumLaunchpad => "raydium_launchpad",
    }
}

#[substreams::handlers::map]
pub fn db_out(mut clock: Clock, swaps: pb::Events) -> Result<DatabaseChanges, Error> {
    let mut tables = substreams_database_change::tables::Tables::new();

    for (transaction_index, transaction) in swaps.transactions.iter().enumerate() {
        for (instruction_index, swap) in transaction.swaps.iter().enumerate() {
            let key: [(&str, String); 3] = common_key_v2(&clock, transaction_index, instruction_index);
            let signers_raw = transaction.signers.iter().map(base58::encode).collect::<Vec<_>>().join(",");
            let row = tables
                .create_row("swaps", key)
                // Transaction
                .set("signature", base58::encode(&transaction.signature))
                .set("fee_payer", base58::encode(&transaction.fee_payer))
                .set("signers_raw", signers_raw)
                .set("fee", transaction.fee)
                .set("compute_units_consumed", transaction.compute_units_consumed)
                .set("program_id", base58::encode(&swap.program_id))
                .set("stack_height", swap.stack_height)

                // Swap
                .set("protocol", protocol_slug(swap.protocol))
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
