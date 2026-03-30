mod decode;
mod normalize;

use proto::pb::{dex::swaps::v1 as pb, jupiter::v1 as jupiter_pb};
use substreams::errors::Error;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    let mut transactions = Vec::new();

    for tx in block.transactions_owned() {
        if let Some(transaction) = decode::decode_boop_transaction(&tx).and_then(normalize::map_boop_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_darklake_transaction(&tx).and_then(normalize::map_darklake_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_dumpfun_transaction(&tx).and_then(normalize::map_dumpfun_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_jupiter_v4_transaction(&tx)
            .and_then(|tx: jupiter_pb::Transaction| normalize::map_jupiter_transaction(tx, decode::PROTOCOL_JUPITER_V4))
        {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_jupiter_v6_transaction(&tx)
            .and_then(|tx: jupiter_pb::Transaction| normalize::map_jupiter_transaction(tx, decode::PROTOCOL_JUPITER_V6))
        {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_meteora_daam_transaction(&tx).and_then(normalize::map_meteora_daam_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_meteora_dllm_transaction(&tx).and_then(normalize::map_meteora_dllm_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_orca_transaction(&tx).and_then(normalize::map_orca_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_pumpfun_transaction(&tx).and_then(normalize::map_pumpfun_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_pumpfun_amm_transaction(&tx).and_then(normalize::map_pumpfun_amm_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_raydium_amm_v4_transaction(&tx).and_then(normalize::map_raydium_amm_v4_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_raydium_clmm_transaction(&tx).and_then(normalize::map_raydium_clmm_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_raydium_cpmm_transaction(&tx).and_then(normalize::map_raydium_cpmm_transaction) {
            transactions.push(transaction);
        }
        if let Some(transaction) = decode::decode_raydium_launchpad_transaction(&tx).and_then(normalize::map_raydium_launchpad_transaction) {
            transactions.push(transaction);
        }
    }

    Ok(pb::Events { transactions })
}
