// mod decode;
mod jupiter_v4;
mod jupiter_v6;
mod meteora_dllm;
mod orca_whirlpool;
mod pumpfun;
mod pumpfun_amm;
mod raydium_amm_v4;
mod raydium_clmm;
mod raydium_cpmm;
mod raydium_launchpad;

use common::solana::{get_fee_payer, get_signers};
use proto::pb::dex::swaps::v1 as pb;
use substreams::errors::Error;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction};

use crate::jupiter_v4::decode_jupiter_v4_transaction;
use crate::jupiter_v6::decode_jupiter_v6_transaction;
use crate::meteora_dllm::decode_meteora_dllm_transaction;
use crate::orca_whirlpool::decode_orca_whirlpool_transaction;
use crate::pumpfun::decode_pumpfun_transaction;
use crate::pumpfun_amm::decode_pumpfun_amm_transaction;
use crate::raydium_amm_v4::decode_raydium_amm_v4_transaction;
use crate::raydium_clmm::decode_raydium_clmm_transaction;
use crate::raydium_cpmm::decode_raydium_cpmm_transaction;
use crate::raydium_launchpad::decode_raydium_launchpad_transaction;

pub(crate) const SOL_MINT: [u8; 32] = [
    6, 155, 136, 87, 254, 171, 129, 132, 251, 104, 127, 99, 70, 24, 192, 53, 218, 196, 57, 220,
    26, 235, 59, 85, 152, 160, 240, 0, 0, 0, 0, 1,
]; // So11111111111111111111111111111111111111111

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: block.transactions_owned().filter_map(process_transaction).collect(),
    })
}

fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let mut swaps = Vec::new();

    swaps.extend(decode_jupiter_v6_transaction(&tx));
    swaps.extend(decode_jupiter_v4_transaction(&tx));
    swaps.extend(decode_meteora_dllm_transaction(&tx));
    swaps.extend(decode_orca_whirlpool_transaction(&tx));
    swaps.extend(decode_pumpfun_transaction(&tx));
    swaps.extend(decode_pumpfun_amm_transaction(&tx));
    swaps.extend(decode_raydium_amm_v4_transaction(&tx));
    swaps.extend(decode_raydium_clmm_transaction(&tx));
    swaps.extend(decode_raydium_cpmm_transaction(&tx));
    swaps.extend(decode_raydium_launchpad_transaction(&tx));

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(&tx).unwrap_or_default(),
        signers: get_signers(&tx).unwrap_or_default(),
        swaps,
    })
}
