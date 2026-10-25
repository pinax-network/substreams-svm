// Native consumers only: dex-swaps links wasm-bindgen glue that the plain
// `wasm/rust-v1` binary type cannot instantiate, and a second `map_events`.
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

use common::db::{common_key_v2, set_clock};
use proto::pb::dex::swaps::v1 as pb;
use substreams::{buffa::Message, errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_solana::base58;

fn protocol_slug(protocol: substreams::buffa::EnumValue<pb::Protocol>) -> &'static str {
    match protocol.as_known().unwrap_or(pb::Protocol::Unspecified) {
        pb::Protocol::Unspecified => "unspecified",
        pb::Protocol::Boop => "boop",
        pb::Protocol::Byreal => "byreal",
        pb::Protocol::Darklake => "darklake",
        pb::Protocol::Dumpfun => "dumpfun",
        pb::Protocol::JupiterV4 => "jupiter_v4",
        pb::Protocol::JupiterV6 => "jupiter_v6",
        pb::Protocol::MeteoraAmm => "meteora_amm",
        pb::Protocol::MeteoraDaam => "meteora_daam",
        pb::Protocol::MeteoraDlmm => "meteora_dlmm",
        pb::Protocol::Moonshot => "moonshot",
        pb::Protocol::OkxDex => "okx_dex",
        pb::Protocol::OrcaWhirlpool => "orca_whirlpool",
        pb::Protocol::Pancakeswap => "pancakeswap",
        pb::Protocol::SplTokenSwap => "spl_token_swap",
        pb::Protocol::Pumpfun => "pumpfun",
        pb::Protocol::PumpfunAmm => "pumpfun_amm",
        pb::Protocol::RaydiumAmmV4 => "raydium_amm_v4",
        pb::Protocol::RaydiumClmm => "raydium_clmm",
        pb::Protocol::RaydiumCpmm => "raydium_cpmm",
        pb::Protocol::RaydiumLaunchpad => "raydium_launchpad",
    }
}

#[cfg(target_arch = "wasm32")]
#[substreams::handlers::map]
pub fn db_out(clock: Clock, swaps: pb::Events) -> Result<DatabaseChanges, Error> {
    changes(&clock, &swaps)
}

/// The unchanged legacy formatter, without decoding or a WASM host call.
/// Native consumers should prefer `native::changes` for clock and size checks.
pub fn changes(clock: &Clock, swaps: &pb::Events) -> Result<DatabaseChanges, Error> {
    changes_with_limit(clock, swaps, None)
}

fn changes_with_limit(clock: &Clock, swaps: &pb::Events, max_bytes: Option<usize>) -> Result<DatabaseChanges, Error> {
    if clock.timestamp.is_unset() {
        return Err(Error::msg("swap formatter requires a clock timestamp"));
    }
    let mut changes = DatabaseChanges::default();
    let mut encoded_bytes = 0usize;

    // Materialize at most one candidate row beyond the accepted output budget.
    // In particular, transaction fields must not expand across every swap in a
    // Tables map before the native caller gets a chance to check their size.
    let mut append = |tables: substreams_database_change::tables::Tables| -> Result<(), Error> {
        let mut candidate = tables.to_database_changes();
        debug_assert_eq!(candidate.table_changes.len(), 1);
        candidate.table_changes[0].ordinal = changes.table_changes.len() as u64;
        // The one-row wrapper includes its repeated-field tag and length prefix.
        // Summing these lengths is the exact length of the final DatabaseChanges,
        // including ordinal varint growth (not just the contained row lengths).
        encoded_bytes = encoded_bytes
            .checked_add(candidate.try_encoded_len()? as usize)
            .ok_or_else(|| Error::msg("swap formatting output capacity exceeded"))?;
        if max_bytes.is_some_and(|limit| encoded_bytes > limit) {
            return Err(Error::msg("swap formatting output capacity exceeded"));
        }
        changes.table_changes.append(&mut candidate.table_changes);
        Ok(())
    };

    for (transaction_index, transaction) in swaps.transactions.iter().enumerate() {
        // Encode once per transaction. Each row still owns the exact legacy
        // field, but only rows within the output budget can accumulate.
        let signers_raw = transaction.signers.iter().map(base58::encode).collect::<Vec<_>>().join(",");
        for (instruction_index, swap) in transaction.swaps.iter().enumerate() {
            let mut tables = substreams_database_change::tables::Tables::new();
            let key: [(&str, String); 3] = common_key_v2(clock, transaction_index, instruction_index);
            let row = tables
                .create_row("swaps", key)
                // Transaction
                .set("signature", base58::encode(&transaction.signature))
                .set("fee_payer", base58::encode(&transaction.fee_payer))
                .set("signers_raw", &signers_raw)
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

            set_clock(clock, row);
            append(tables)?;
        }
    }

    if swaps.transactions.iter().any(|transaction| !transaction.swaps.is_empty()) {
        let mut tables = substreams_database_change::tables::Tables::new();
        set_clock(clock, tables.create_row("blocks", [("block_num", clock.number.to_string())]));
        append(tables)?;
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_slug_preserves_known_values_and_unknown_numeric_fallback() {
        assert_eq!(protocol_slug(pb::Protocol::MeteoraDlmm.into()), "meteora_dlmm");
        assert_eq!(protocol_slug(pb::Protocol::Unspecified.into()), "unspecified");
        assert_eq!(protocol_slug(999_i32.into()), "unspecified");
    }
}
