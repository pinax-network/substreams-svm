use super::*;
use dex_swaps::SwapKind;
use flate2::read::GzDecoder;
use std::io::Read;
use substreams_solana::pb::sf::solana::r#type::v1 as sol;

fn fixture() -> sol::Block {
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../../../dex-swaps/fixtures/solana-mainnet-350000026.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    sol::Block::decode_from_slice(&bytes).unwrap()
}
fn clock(block: &sol::Block) -> Clock {
    let mut clock = Clock {
        number: block.slot,
        id: block.blockhash.clone(),
        ..Default::default()
    };
    clock.timestamp.get_or_insert_default().seconds = block.block_time.timestamp;
    clock
}
fn canonical(mut rows: DatabaseChanges) -> DatabaseChanges {
    for row in &mut rows.table_changes {
        row.fields.sort_by(|a, b| a.name.cmp(&b.name));
    }
    rows
}
#[test]
fn complete_block_matches_wrapper_rows_keys_ordinals_and_unqualified_summaries() {
    let block = fixture();
    let projection = dex_swaps::project(&block).unwrap();
    assert!(projection
        .transactions
        .iter()
        .flat_map(|tx| &tx.swaps)
        .any(|row| row.kind == SwapKind::AggregatorSummary));
    assert!(projection
        .transactions
        .iter()
        .flat_map(|tx| &tx.swaps)
        .any(|row| row.verified_context.is_none()));
    let expected = crate::changes(&clock(&block), &dex_swaps::map_events(block.clone()).unwrap()).unwrap();
    let actual = changes(&clock(&block), projection, 100_000, 64 * 1024 * 1024).unwrap();
    assert_eq!(actual.table_changes.iter().filter(|row| row.table == "swaps").count(), 231);
    assert_eq!(canonical(actual), canonical(expected));
}
#[test]
fn clock_count_and_exact_encoded_output_bounds_fail_atomically() {
    let block = fixture();
    let projection = dex_swaps::project(&block).unwrap();
    let clock = clock(&block);
    for kind in 0..5 {
        let mut bad = clock.clone();
        match kind {
            0 => bad.number += 1,
            1 => bad.id = "different".into(),
            2 => bad.timestamp.get_or_insert_default().nanos = 1,
            3 => bad.timestamp = Default::default(),
            4 => bad.timestamp.get_or_insert_default().seconds += 1,
            _ => unreachable!(),
        }
        assert!(changes(&bad, projection.clone(), 100_000, 64 * 1024 * 1024).is_err());
    }
    for (count, bytes) in [
        (0, 64 * 1024 * 1024),
        (230, 64 * 1024 * 1024),
        (100_001, 64 * 1024 * 1024),
        (100_000, 0),
        (100_000, 64 * 1024 * 1024 + 1),
    ] {
        assert!(changes(&clock, projection.clone(), count, bytes).is_err());
    }
    let actual = changes(&clock, projection.clone(), 231, 64 * 1024 * 1024).unwrap();
    let size = actual.try_encoded_len().unwrap() as usize;
    assert!(changes(&clock, projection.clone(), 231, size).is_ok());
    assert!(changes(&clock, projection, 231, size - 1).is_err());
}

#[test]
fn repeated_signer_fields_are_bounded_while_rows_are_materialized() {
    let block = fixture();
    let mut projection = dex_swaps::project(&block).unwrap();
    projection.transactions.truncate(1);
    let tx = &mut projection.transactions[0];
    // Every row repeats this signer list in base58, so one row is nearly the
    // whole input-sized budget and the batch would be over 200 times it. The
    // formatter must stop at the budget instead of building the whole batch and
    // rejecting it later (as 9884b99 did).
    tx.signers = vec![vec![1; 32]; 4096];
    tx.swaps = vec![tx.swaps[0].clone(); 256];
    let clock = clock(&block);
    let max_bytes = projection.clone().into_legacy().try_encoded_len().unwrap() as usize + clock.try_encoded_len().unwrap() as usize;
    let mut one = projection.clone();
    one.transactions[0].swaps.truncate(1);
    let mut row = changes(&clock, one, 1, 64 * 1024 * 1024).unwrap();
    row.table_changes.truncate(1);
    let row_bytes = row.try_encoded_len().unwrap() as usize;
    assert!(256 * row_bytes > 200 * max_bytes);
    crate::MATERIALIZED_ROWS.with(|rows| rows.set(0));
    let error = changes(&clock, projection, 256, max_bytes).unwrap_err();
    assert!(error.to_string().contains("output capacity exceeded"));
    // Only the rows that fit, plus the one rejected candidate.
    let materialized = crate::MATERIALIZED_ROWS.with(|rows| rows.get());
    assert!((1..=max_bytes / row_bytes + 1).contains(&materialized), "{materialized} rows materialized");
}
#[test]
fn dense_legacy_indexes_cannot_be_shifted_by_empty_duplicate_or_reordered_transactions() {
    let block = fixture();
    let projection = dex_swaps::project(&block).unwrap();
    for kind in 0..4 {
        let mut bad = projection.clone();
        match kind {
            0 => bad.transactions[0].swaps.clear(),
            1 => bad.transactions.swap(0, 1),
            2 => bad.transactions[0].transaction_index = bad.clock.transaction_count,
            3 => bad.transactions[1].transaction_index = bad.transactions[0].transaction_index,
            _ => unreachable!(),
        }
        assert!(changes(&clock(&block), bad, 100_000, 64 * 1024 * 1024).is_err());
    }
}
#[test]
fn empty_complete_block_has_no_database_rows() {
    let mut block = fixture();
    block.transactions.clear();
    let projection = dex_swaps::project(&block).unwrap();
    assert!(changes(&clock(&block), projection, 1, 1024).unwrap().table_changes.is_empty());
}

mod legacy_v051;
