//! Runs the WASM embedded in the committed `spkg/` files, as wired in each
//! package, and requires the native rows. A package packed before a decoder or
//! formatter change fails here: bump the manifest version and repack it under
//! the new name. Released files are pinned by SHA-256 and never repacked in
//! place. See `docs/releasing.md`.
use super::*;
use package_wasm::{database_digest, Manifest, Package, Sources};
use std::path::{Path, PathBuf};
use substreams_solana::base58;

#[path = "../../../../svm-transfers/tests/support/package_wasm.rs"]
mod package_wasm;

/// Manifests whose committed packages these tests cover, in pack order.
const MANIFESTS: [&str; 3] = ["dex-swaps", "svm-dex", "svm-dex/clickhouse"];

/// Files attached to the svm-dex v0.5.3 release. A changed file must get a
/// new version.
const RELEASED: [(&str, &str); 3] = [
    ("dex-swaps-v0.5.3.spkg", "236f35e0c292fb31d1e5180daf9f00008d3717c239f6a975592a3b06857edcc7"),
    ("svm-dex-v0.5.3.spkg", "9c43c9278e52e734f99f15eedb5a6ba7a3ad5c349ce25966a3702709c5eab86e"),
    (
        "svm-clickhouse-dex-v0.5.3.spkg",
        "73fcd3aefaa04b6c405c6d176dfd12ee8ec960e047f521f1f308cdb59b5b8af1",
    ),
];

/// Frozen output of the released svm-dex v0.5.3 package: slot, swaps rows
/// (v1), all rows, and `database_digest`.
const GOLDEN_V053: [(u64, (usize, usize), usize, &str); 5] = [
    (349019252, (251, 0), 252, "68da8e2bb762e5747d01dd6f4178e1ba11f7cfa3f20585415bbf234d9339577c"),
    (350000009, (180, 0), 181, "ae953e4c864538a3174c47887bbb791f561975838735faef06da25720b31d8ec"),
    (350000010, (155, 0), 156, "3b12fac0a081b479a6c2c2d1d721c6f9be285fdc42cd75ce25c62759916e2e44"),
    (350000026, (231, 0), 232, "fe113183e26c46245ea846e993600961587519abb22bbf1c1451dbcf80a1e584"),
    (461335700, (61, 10), 62, "3286cf4db90c9344cf1164c127b79ad1b950028ddba15c60f70f88f6c8ae1287"),
];

/// The only single-transaction case where the package and native differ: the
/// package's `blocks_without_votes` input drops a transaction with a static
/// `Vote111…` key, native keeps its swap.
const VOTE_KEY_CASE: &str = "mainnet_swap_transaction_with_vote_key";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The committed package of a manifest at its current version.
fn committed(dir: &str) -> Package {
    Package::open(root().join("spkg").join(Manifest::read(root().join(dir)).default_spkg_name()))
}

fn mainnet_blocks() -> Vec<sol::Block> {
    [
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-349019252.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-350000009.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-350000010.pb.gz").as_slice(),
        include_bytes!("../../../../dex-swaps/fixtures/solana-mainnet-350000026.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz").as_slice(),
    ]
    .into_iter()
    .map(|fixture| {
        let mut bytes = Vec::new();
        GzDecoder::new(fixture).read_to_end(&mut bytes).unwrap();
        sol::Block::decode_from_slice(&bytes).unwrap()
    })
    .collect()
}

fn packed(package: &Package, block: &sol::Block) -> DatabaseChanges {
    let clock = clock(block).encode_to_vec();
    let output = package
        .evaluate(
            "db_out",
            &Sources {
                block: &block.encode_to_vec(),
                clock: &clock,
                params: &[],
            },
        )
        .unwrap();
    canonical(DatabaseChanges::decode_from_slice(&output).unwrap())
}

fn native(block: &sol::Block) -> DatabaseChanges {
    canonical(changes(&clock(block), dex_swaps::project(block).unwrap(), 100_000, 64 * 1024 * 1024).unwrap())
}

/// `swaps` rows, and how many of them belong to v1 transactions.
fn swaps(block: &sol::Block, changes: &DatabaseChanges) -> (usize, usize) {
    let v1: std::collections::BTreeSet<_> = block
        .transactions
        .iter()
        .filter(|tx| tx.transaction.message.version == Some(1))
        .map(|tx| base58::encode(&tx.transaction.signatures[0]))
        .collect();
    let rows: Vec<_> = changes.table_changes.iter().filter(|row| row.table == "swaps").collect();
    let v1_rows = rows
        .iter()
        .filter(|row| row.fields.iter().any(|field| field.name == "signature" && v1.contains(&field.value)))
        .count();
    (rows.len(), v1_rows)
}

#[test]
fn committed_dex_packages_match_their_manifests() {
    let errors = package_wasm::check_committed_packages(&root(), &MANIFESTS, &RELEASED);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn committed_svm_dex_package_matches_native_rows() {
    let package = committed("svm-dex");
    let released = package.path.ends_with("svm-dex-v0.5.3.spkg");
    assert_eq!(
        package.wiring("dex_swaps:map_events", "dex_swaps:"),
        "map_events <- [map solana_common:blocks_without_votes]"
    );
    assert_eq!(
        package.wiring("db_out", ""),
        "db_out <- [source sf.substreams.v1.Clock, map dex_swaps:map_events]"
    );
    for (index, block) in mainnet_blocks().iter().enumerate() {
        let actual = packed(&package, block);
        let observed = (block.slot, swaps(block, &actual), actual.table_changes.len(), database_digest(&actual));
        eprintln!("  {observed:?}");
        assert!(
            actual == native(block),
            "slot {}: the committed package differs from native::changes",
            block.slot
        );
        if released {
            assert_eq!((observed.0, observed.1, observed.2, observed.3.as_str()), GOLDEN_V053[index]);
        }
    }
    let mut counts = Vec::new();
    for case in super::legacy_v051::block_cases() {
        let (actual, expected) = (packed(&package, &case.block), native(&case.block));
        counts.push((case.name, swaps(&case.block, &actual).0, swaps(&case.block, &expected).0));
        if case.name == VOTE_KEY_CASE {
            assert!(actual.table_changes.is_empty() && swaps(&case.block, &expected).0 == 1, "{}", case.name);
        } else {
            assert!(actual == expected, "{}: the committed package differs from native::changes", case.name);
        }
    }
    eprintln!("  {counts:?}");
    assert!(counts.iter().any(|case| case.0 == VOTE_KEY_CASE));
}

#[test]
fn committed_svm_dex_db_out_matches_the_formatter_on_synthetic_events() {
    let package = committed("svm-dex");
    let clock = clock(&fixture());
    let cases = super::legacy_v051::synthetic();
    for (name, events) in &cases {
        let packed = package
            .run("db_out", &[&clock.encode_to_vec(), &events.encode_to_vec()])
            .map(|output| canonical(DatabaseChanges::decode_from_slice(&output).unwrap()));
        assert_eq!(packed, crate::changes(&clock, events).ok().map(canonical), "{name}");
    }
    assert_eq!(cases.len(), 27);
}

#[test]
fn committed_clickhouse_dex_package_embeds_the_same_graph() {
    let core = committed("svm-dex");
    let sink = committed("svm-dex/clickhouse");
    assert_eq!(sink.binary_of("db_out"), core.binary_of("db_out"));
    assert_eq!(sink.wiring("db_out", "db:"), core.wiring("db_out", ""));
    let block = mainnet_blocks().pop().unwrap();
    assert_eq!(packed(&sink, &block), packed(&core, &block));
}
