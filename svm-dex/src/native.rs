//! Legacy rows from an already computed native swap projection.
use dex_swaps::Projection;
use substreams::{buffa::Message, errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;

/// Consume one complete projection after any typed consumer has borrowed it.
/// No protobuf block decode or protocol extraction is repeated. Input vectors
/// and fields move into the legacy view; source/provenance additions are dropped.
/// Do not filter or reorder the projection before this call. In particular,
/// legacy order and aggregator summaries remain unchanged regardless of native
/// invocation qualification or evidence execution order.
pub fn changes(clock: &Clock, projection: Projection, max_swaps: usize, max_bytes: usize) -> Result<DatabaseChanges, Error> {
    let time = clock.timestamp.as_option().ok_or_else(|| Error::msg("missing swap clock"))?;
    if clock.number != projection.clock.slot
        || clock.id != projection.clock.blockhash
        || Some(time.seconds) != projection.clock.block_time
        || time.nanos != 0
        || !(0..=253_402_300_799).contains(&time.seconds)
    {
        return Err(Error::msg("mismatched swap projection clock"));
    }
    if max_swaps == 0 || max_swaps > 100_000 || max_bytes == 0 || max_bytes > 64 * 1024 * 1024 {
        return Err(Error::msg("invalid swap formatting bounds"));
    }
    let mut count = 0usize;
    let mut previous = None;
    for tx in &projection.transactions {
        // Empty transactions never occur in the complete native projection.
        // Refuse to let fabricated or reordered entries shift dense legacy keys.
        if tx.swaps.is_empty() || tx.transaction_index >= projection.clock.transaction_count || previous.is_some_and(|index| tx.transaction_index <= index) {
            return Err(Error::msg("invalid swap projection transaction order"));
        }
        previous = Some(tx.transaction_index);
        count = count.saturating_add(tx.swaps.len());
        if count > max_swaps {
            return Err(Error::msg("swap formatting count capacity exceeded"));
        }
    }
    // Conversion only moves existing fields; it does not clone their buffers.
    // Count bounds above also bound the new transaction/swap vector allocations.
    let events = projection.into_legacy();
    if events.try_encoded_len()?.saturating_add(clock.try_encoded_len()?) as usize > max_bytes {
        return Err(Error::msg("swap formatting input capacity exceeded"));
    }
    crate::changes_with_limit(clock, &events, Some(max_bytes))
}

#[cfg(test)]
mod tests;
