#![allow(dead_code)]

use flate2::read::GzDecoder;
use prost::Message;
use std::io::Read;
pub use substreams_solana::pb::sf::solana::r#type::v1 as pb;

spl_transfers::block_view!(pub fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

// Complete mainnet blocks fetched from solana.firehose.pinax.network; see
// fixtures/README.md for provenance and checksums.

/// Slot 349019252, whose parent is 349019247: slots 349019248..=349019251 were
/// skipped. Includes Token-2022 operations.
pub const BLOCK_349019252: &[u8] = include_bytes!("../fixtures/solana-mainnet-349019252.pb.gz");
/// Slot 350000009: a top-level zero-amount `TransferChecked` (transaction 1366).
pub const BLOCK_350000009: &[u8] = include_bytes!("../fixtures/solana-mainnet-350000009.pb.gz");
/// Slot 350000010: failed transactions 22 and 119 with top-level `TransferChecked`.
pub const BLOCK_350000010: &[u8] = include_bytes!("../fixtures/solana-mainnet-350000010.pb.gz");
/// Slot 350000026: a mint from a loaded address (776), a temporary account
/// (1282), extra accounts (1337) and an inner self transfer (1335).
pub const BLOCK_350000026: &[u8] = include_bytes!("../fixtures/solana-mainnet-350000026.pb.gz");
/// Slot 461335700 (2026-10-25, after Alpenglow): 132 of its 314 transactions
/// are v1 messages. Re-encoded from public JSON-RPC `getBlock` with the
/// Firehose poller's mapping, not fetched from a provider; see
/// fixtures/rpc/README.md.
pub const BLOCK_461335700: &[u8] = include_bytes!("../fixtures/rpc/solana-mainnet-461335700.pb.gz");

pub fn gunzip(fixture: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    GzDecoder::new(fixture).read_to_end(&mut bytes).expect("gzip fixture");
    bytes
}

pub fn decode_block(fixture: &[u8]) -> pb::Block {
    pb::Block::decode(gunzip(fixture).as_slice()).expect("sf.solana.type.v1.Block fixture")
}

pub fn b58(bytes: impl AsRef<[u8]>) -> String {
    bs58::encode(bytes).into_string()
}
