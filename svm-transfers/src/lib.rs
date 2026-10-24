pub mod native;
mod native_token;
mod spl_token;

use common::{db::set_clock, solana::update_genesis_clock};
use proto::pb::solana as pb;
use substreams::{errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;

#[cfg(target_arch = "wasm32")]
#[substreams::handlers::map]
pub fn db_out(clock: Clock, spl_token: pb::spl::token::v1::Events, native_token: pb::native::token::v1::Events) -> Result<DatabaseChanges, Error> {
    let changes = changes(&clock, &spl_token, &native_token)?;
    substreams::log::info!("Total rows {}", changes.table_changes.len());
    Ok(changes)
}

/// The same database formatter as `db_out`, without a WASM handler or host calls.
/// Callers bound input events and output size; this function does no extraction.
pub fn changes(clock: &Clock, spl_token: &pb::spl::token::v1::Events, native_token: &pb::native::token::v1::Events) -> Result<DatabaseChanges, Error> {
    if clock.timestamp.is_unset() {
        return Err(Error::msg("transfer formatter requires a clock timestamp"));
    }
    let clock = update_genesis_clock(clock.clone());
    let mut tables = substreams_database_change::tables::Tables::new();

    spl_token::process_events(&mut tables, &clock, spl_token);
    native_token::process_events(&mut tables, &clock, native_token);

    // ONLY include blocks if events are present
    if tables.all_row_count() > 0 {
        set_clock(&clock, tables.create_row("blocks", [("block_num", clock.number.to_string())]));
    }
    Ok(tables.to_database_changes())
}
