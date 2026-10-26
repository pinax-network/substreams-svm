//! Runs the WASM embedded in the committed `spkg/` files, as wired in each
//! package, and requires the native rows. A package packed before a decoder or
//! formatter change fails here: bump the manifest version and repack it under
//! the new name. Released files are pinned by SHA-256 and never repacked in
//! place. See `docs/releasing.md`.
use super::*;
use package_wasm::{database_digest, Manifest, Package, Sources};
use std::path::{Path, PathBuf};

#[path = "../../../tests/support/package_wasm.rs"]
mod package_wasm;

/// Manifests whose committed packages these tests cover, in pack order.
const MANIFESTS: [&str; 5] = [
    "spl/token",
    "native/system",
    "spl/transfers-substreams",
    "svm-transfers",
    "svm-transfers/clickhouse",
];

/// Files attached to the svm-transfers v0.4.0 release, plus the committed,
/// unreleased spl_transfers v0.1.0. A changed file must get a new version.
const RELEASED: [(&str, &str); 5] = [
    ("svm-transfers-v0.4.0.spkg", "1ff849bea5a39623ce07af9d2c7cf007acfe6d94228832880f3cf7e780e320f1"),
    (
        "svm-clickhouse-transfers-v0.4.0.spkg",
        "c876f72a190fa2a1739cb841ca6ed22f7a2dd991673362bc2b7ddd57444c786f",
    ),
    ("spl-token-v0.4.0.spkg", "b547c1a3d98a2cff0183f92d7dbfd50cd120fb157e3e4f901b68f75b861c9172"),
    ("native-token-v0.3.1.spkg", "daa96560cdc600bf9fc9c70bebef0fbf155e4d4b69f0045fd4bb2b7b133c9b61"),
    ("spl-transfers-v0.1.0.spkg", "8cb7a3fcc5e406d502c2e0bd6fd2f277a8bfb4ff9a0638a2923376d567b29811"),
];

/// Frozen output of the released svm-transfers v0.4.0 package: slot, SPL rows
/// (v1), system rows (v1), all rows, and `database_digest`.
const GOLDEN_V040: [(u64, (usize, usize), (usize, usize), usize, &str); 5] = [
    (
        349019252,
        (668, 0),
        (477, 0),
        1146,
        "49470254eb71cfc5e7f21fa376b8f06119aacc7f8f19c4696ce48b51a29a8711",
    ),
    (
        350000009,
        (469, 0),
        (174, 0),
        644,
        "c3f3ec18aa50c13b6cd2ec3310fc1a800f90a4dda7423e0b02cf55abf1e7b94a",
    ),
    (
        350000010,
        (407, 0),
        (242, 0),
        650,
        "66c69dbe1000523bdbf3e3a7d04c4d2a614ae4a2c9e8ab8f6d153d81279ac6c1",
    ),
    (
        350000026,
        (602, 0),
        (366, 0),
        969,
        "fb041354407f2093f9799f615decc5ac42c355910d4b77ccba70af17cd2f101b",
    ),
    (
        461335700,
        (296, 59),
        (104, 27),
        401,
        "b498b47203034aba70e10f87df69287c03702e848e396fcde64870dc89a9cfd9",
    ),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The committed package of a manifest at its current version.
fn committed(dir: &str) -> Package {
    Package::open(root().join("spkg").join(Manifest::read(root().join(dir)).default_spkg_name()))
}

fn mainnet_blocks() -> Vec<(Vec<u8>, sol::Block)> {
    [
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-349019252.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-350000009.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-350000010.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/solana-mainnet-350000026.pb.gz").as_slice(),
        include_bytes!("../../../../spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz").as_slice(),
    ]
    .into_iter()
    .map(|fixture| {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fixture).read_to_end(&mut bytes).unwrap();
        let block = sol::Block::decode_from_slice(&bytes).unwrap();
        (bytes, block)
    })
    .collect()
}

fn packed(package: &Package, bytes: &[u8], block: &sol::Block) -> Result<DatabaseChanges, String> {
    let clock = clock(block).encode_to_vec();
    let output = package.evaluate(
        "db_out",
        &Sources {
            block: bytes,
            clock: &clock,
            params: &[],
        },
    )?;
    Ok(canonical(DatabaseChanges::decode_from_slice(&output).unwrap()))
}

/// Rows of `table`, and how many of them belong to v1 transactions.
fn count(block: &sol::Block, changes: &DatabaseChanges, table: &str) -> (usize, usize) {
    let v1: std::collections::BTreeSet<_> = block
        .transactions
        .iter()
        .filter(|tx| tx.transaction.message.version == Some(1))
        .map(|tx| base58::encode(&tx.transaction.signatures[0]))
        .collect();
    let rows: Vec<_> = changes.table_changes.iter().filter(|row| row.table == table).collect();
    let v1_rows = rows
        .iter()
        .filter(|row| row.fields.iter().any(|field| field.name == "signature" && v1.contains(&field.value)))
        .count();
    (rows.len(), v1_rows)
}

#[test]
fn committed_transfers_packages_match_their_manifests() {
    let errors = package_wasm::check_committed_packages(&root(), &MANIFESTS, &RELEASED);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn committed_svm_transfers_package_matches_native_rows() {
    let package = committed("svm-transfers");
    let released = package.path.ends_with("svm-transfers-v0.4.0.spkg");
    // spl_token reads the unfiltered block and native_token the vote-filtered
    // one; both keep their block filters.
    let spl = package.wiring("spl_token:map_events", "spl_token:");
    assert!(
        spl.starts_with("map_events <- [source sf.solana.type.v1.Block] filter solana_common:program_ids_without_votes"),
        "{spl}"
    );
    for program in [
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
        "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
    ] {
        assert!(spl.contains(&format!("program:{program}")), "{spl}");
    }
    assert!(package
        .wiring("native_token:map_events", "native_token:")
        .starts_with("map_events <- [map solana_common:blocks_without_votes] filter solana_common:program_ids_without_votes"));
    for (index, (bytes, block)) in mainnet_blocks().iter().enumerate() {
        let actual = packed(&package, bytes, block).unwrap();
        let expected = canonical(formatted(block));
        let observed = (
            block.slot,
            count(block, &actual, "spl_transfer"),
            count(block, &actual, "system_transfer"),
            actual.table_changes.len(),
            database_digest(&actual),
        );
        eprintln!("  {observed:?}");
        assert!(actual == expected, "slot {}: the committed package differs from native::changes", block.slot);
        if released {
            let golden = GOLDEN_V040[index];
            assert_eq!((observed.0, observed.1, observed.2, observed.3, observed.4.as_str()), golden);
        }
    }
}

/// The v0.3.0 boundary corpus through the committed package. Native rejects
/// four source shapes the package skips (it keeps their system rows); in a
/// single-transaction block a Vote-key transaction leaves no Token program in
/// the vote-filtered index, so the package writes no rows, like v0.3.0, while
/// native keeps them. Every other case equals native.
#[test]
fn committed_svm_transfers_package_on_the_v030_boundary_corpus() {
    const NATIVE_REJECTS: [&str; 4] = [
        "malformed_system",
        "unsupported_message_version",
        "v2_message",
        "v1_message_with_address_table_lookups",
    ];
    const VOTE_KEY: [&str; 2] = ["vote_key_transfers", "vote_instruction_and_transfers"];
    let package = committed("svm-transfers");
    let oracle = super::legacy_v030::oracle();
    let mut equal = 0;
    let cases = super::legacy_v030::cases();
    for (name, block) in &cases {
        let bytes = block.encode_to_vec();
        let actual = packed(&package, &bytes, block).unwrap();
        let native = view(block).ok().and_then(|view| {
            let spl = spl_transfers::project(&view, Coverage::ALL).ok()?;
            let system = native_token::project(block).ok()?;
            changes(&clock(block), &spl, &system, 100_000, 64 * 1024 * 1024).ok().map(canonical)
        });
        let legacy = oracle
            .database(&clock(block).encode_to_vec(), &bytes)
            .map(|output| canonical(DatabaseChanges::decode_from_slice(&output).unwrap()));
        let system = |changes: &DatabaseChanges| {
            let mut changes = changes.clone();
            // Ordinals count the rows written before, including SPL rows.
            changes.table_changes.retain(|row| row.table.starts_with("system_"));
            changes.table_changes.iter_mut().for_each(|row| row.ordinal = 0);
            changes
        };
        eprintln!(
            "  {name:40} packed {:?} {:?} | native {:?} | v0.3.0 {:?}",
            count(block, &actual, "spl_transfer"),
            count(block, &actual, "system_transfer"),
            native.as_ref().map(|n| n.table_changes.len()),
            legacy.as_ref().map(|l| l.table_changes.len())
        );
        if NATIVE_REJECTS.contains(name) {
            assert!(native.is_none(), "{name}: native accepts");
            assert_eq!(count(block, &actual, "spl_transfer").0, 0, "{name}");
            if let Some(legacy) = &legacy {
                assert_eq!(system(&actual), system(legacy), "{name}: system rows differ from v0.3.0");
            }
        } else if VOTE_KEY.contains(name) {
            assert!(actual.table_changes.is_empty(), "{name}");
            assert!(legacy.as_ref().is_some_and(|legacy| legacy.table_changes.is_empty()), "{name}");
            assert!(native.as_ref().is_some_and(|native| !native.table_changes.is_empty()), "{name}");
        } else {
            assert_eq!(Some(&actual), native.as_ref(), "{name}");
            equal += 1;
        }
    }
    assert_eq!(equal, cases.len() - NATIVE_REJECTS.len() - VOTE_KEY.len());
}

#[test]
fn committed_clickhouse_transfers_package_embeds_the_same_graph() {
    let core = committed("svm-transfers");
    let sink = committed("svm-transfers/clickhouse");
    assert_eq!(sink.binary_of("db_out"), core.binary_of("db_out"));
    assert_eq!(sink.wiring("db_out", "db:"), core.wiring("db_out", ""));
    let (bytes, block) = mainnet_blocks().pop().unwrap();
    assert_eq!(packed(&sink, &bytes, &block), packed(&core, &bytes, &block));
}

/// The typed spl_transfers package with its default params and with `all`.
#[test]
fn committed_spl_transfers_package_matches_native() {
    use proto::pb::solana::spl::transfers::v1 as output;
    let package = committed("spl/transfers-substreams");
    assert_eq!(package.wiring("map_events", ""), "map_events <- [params \"\", source sf.solana.type.v1.Block]");
    let mut counts = Vec::new();
    for (bytes, block) in mainnet_blocks() {
        for params in ["", "all"] {
            let sources = Sources {
                block: &bytes,
                clock: &[],
                params: &[("map_events", params)],
            };
            let actual = output::Events::decode_from_slice(&package.evaluate("map_events", &sources).unwrap()).unwrap();
            let expected = spl_transfers_substreams::map_events_impl(params, &block).unwrap();
            assert!(actual == expected, "slot {} params {params:?}", block.slot);
            counts.push((block.slot, params, actual.operations.len()));
        }
    }
    eprintln!("  {counts:?}");
    assert!(
        counts.contains(&(350000026, "all", 602)) && counts.contains(&(461335700, "all", 296)),
        "{counts:?}"
    );
}
