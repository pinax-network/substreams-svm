// mod decode;
mod jupiter_v4;
mod jupiter_v6;
mod logs;
mod boop;
mod darklake;
mod dumpfun;
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
use substreams_solana::{pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction}};

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
    let mut boop_state = boop::State::new();
    let mut darklake_state = darklake::State::new();
    let mut dumpfun_state = dumpfun::State::new();
    let mut pumpfun_pending = None;
    let mut pumpfun_amm_pending = None;
    let mut meteora_dllm_pending = None;
    let mut raydium_launchpad_pending = None;
    let mut raydium_amm_v4_state = raydium_amm_v4::State::new();
    let mut raydium_clmm_state = raydium_clmm::State::new();
    let mut raydium_cpmm_state = raydium_cpmm::State::new();
    let mut orca_whirlpool_state = orca_whirlpool::State::new();

    for instruction in tx.walk_instructions() {
        if let Some(swap) = jupiter_v6::decode_instruction(&tx, &instruction) {
            swaps.push(swap);
        }
        darklake_state.handle_instruction(&instruction);
        if let Some(swap) = pumpfun::handle_instruction(&mut pumpfun_pending, &instruction) {
            swaps.push(swap);
        }
        if let Some(swap) = pumpfun_amm::handle_instruction(&mut pumpfun_amm_pending, &instruction) {
            swaps.push(swap);
        }
        if let Some(swap) = meteora_dllm::handle_instruction(&mut meteora_dllm_pending, &instruction) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_launchpad::handle_instruction(&mut raydium_launchpad_pending, &instruction) {
            swaps.push(swap);
        }

        raydium_amm_v4_state.handle_instruction(&instruction);
        raydium_clmm_state.handle_instruction(&instruction);
        raydium_cpmm_state.handle_instruction(&instruction);
        orca_whirlpool_state.handle_instruction(&instruction);
    }

    let mut jupiter_v4_state = jupiter_v4::State::new();
    for log_message in tx_meta.log_messages.iter() {
        if let Some(swap) = jupiter_v4_state.handle_log(&tx, log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = boop_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = darklake_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = dumpfun_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_amm_v4_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_clmm_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_cpmm_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = orca_whirlpool_state.handle_log(log_message) {
            swaps.push(swap);
        }
    }

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
