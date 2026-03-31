mod native_token;
mod spl_token;

use proto::pb::solana as pb;
use substreams::{errors::Error, pb::substreams::Clock};
use substreams_database_change::{pb::sf::substreams::sink::database::v1::DatabaseChanges, tables::Row};

#[substreams::handlers::map]
pub fn db_out(
    clock: Clock,
    spl_token: pb::spl::token::v1::Events,
    native_token: pb::native::token::v1::Events,
) -> Result<DatabaseChanges, Error> {
    let mut tables = substreams_database_change::tables::Tables::new();

    spl_token::process_events(&mut tables, &clock, &spl_token);
    native_token::process_events(&mut tables, &clock, &native_token);

    // ONLY include blocks if events are present
    if tables.all_row_count() > 0 {
        set_clock(&clock, tables.create_row("blocks", [("block_num", clock.number.to_string())]));
    }
    substreams::log::info!("Total rows {}", tables.all_row_count());
    Ok(tables.to_database_changes())
}

// Helper function to set clock data in a row
pub fn set_clock(clock: &Clock, row: &mut Row) {
    row.set("block_num", clock.number.to_string())
        .set("block_hash", &clock.id)
        .set("timestamp", clock.timestamp.as_ref().expect("missing timestamp").seconds.to_string());
}
