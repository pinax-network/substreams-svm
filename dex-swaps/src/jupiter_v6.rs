use common::solana::get_fee_payer;
use proto::pb::dex::swaps::v1 as pb;
use substreams::log;
use substreams_solana::{base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::jupiter;

use crate::routed_pool::Tracker;

/// Discriminator for the Jupiter v6 `Swap` self-CPI event log. Mirrors the
/// private constant in `substreams_solana_idls::jupiter::v6::events` so we can
/// build fixtures without exposing it upstream.
#[cfg(test)]
const SWAP_DISCRIMINATOR: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

pub(crate) fn decode_instruction(tx: &ConfirmedTransaction, instruction: &InstructionView, routed_pools: &Tracker) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &jupiter::v6::PROGRAM_ID {
        return None;
    }

    match jupiter::v6::events::unpack(instruction.data()) {
        Ok(jupiter::v6::events::JupiterV6Event::Swap(event)) => {
            let amm_program = event.amm.to_bytes();
            let amm_pool = routed_pools.lookup(&amm_program).cloned().unwrap_or_default();

            if amm_pool.len() > 0 {
                log::info!("V6 ✅ {}", base58::encode(amm_program));
            } else {
                log::info!("V6 ⚠️  {} (unknown pool)", base58::encode(amm_program));
            }
            Some(pb::Swap {
                program_id: jupiter::v6::PROGRAM_ID.to_vec(),
                protocol: pb::Protocol::JupiterV6 as i32,
                stack_height: instruction.stack_height(),
                amm: amm_program.to_vec(),
                amm_pool,
                user: get_fee_payer(tx).unwrap_or_default(),
                input_mint: event.input_mint.to_bytes().to_vec(),
                input_amount: event.input_amount,
                output_mint: event.output_mint.to_bytes().to_vec(),
                output_amount: event.output_amount,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use substreams_solana_idls::pumpfun;

    /// PumpSwap (`pumpfun::amm`) Buy discriminator. Mirrors the private
    /// constant in `substreams_solana_idls::pumpfun::amm::instructions`.
    const PUMPFUN_AMM_BUY: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

    /// Build a Jupiter v6 Swap-event instruction data payload with the given
    /// routed AMM and amounts. Matches the on-chain layout: 8 bytes event
    /// discriminator + 8 bytes Anchor discriminator + 112 bytes Borsh-serialised
    /// `SwapEvent { amm, input_mint, input_amount, output_mint, output_amount }`.
    fn jup_swap_event_data(amm: [u8; 32], input_mint: [u8; 32], input_amount: u64, output_mint: [u8; 32], output_amount: u64) -> Vec<u8> {
        let mut data = SWAP_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&[0u8; 8]); // anchor discriminator (ignored by parser)
        data.extend_from_slice(&amm);
        data.extend_from_slice(&input_mint);
        data.extend_from_slice(&input_amount.to_le_bytes());
        data.extend_from_slice(&output_mint);
        data.extend_from_slice(&output_amount.to_le_bytes());
        data
    }

    #[test]
    fn populates_amm_pool_from_routed_inner_cpi() {
        // Pre-record a PumpSwap pool in the tracker, as Tracker::observe would
        // during the walk.
        let pool = [9u8; 32];
        let mints_and_filler = [[2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], [6u8; 32]];
        let mut accounts = vec![pool];
        accounts.extend_from_slice(&mints_and_filler);
        let mut data = PUMPFUN_AMM_BUY.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes());
        data.extend_from_slice(&50_000u64.to_le_bytes());
        let routed_tx = make_tx(pumpfun::amm::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in routed_tx.walk_instructions() {
            tracker.observe(&ix);
        }

        // Build a Jupiter v6 Swap event referencing the routed program.
        let input_mint = [11u8; 32];
        let output_mint = [12u8; 32];
        let event_data = jup_swap_event_data(pumpfun::amm::PROGRAM_ID, input_mint, 1_000, output_mint, 900);
        let jup_tx = make_tx(jupiter::v6::PROGRAM_ID, &[], event_data);

        let instruction = jup_tx.walk_instructions().next().unwrap();
        let swap = decode_instruction(&jup_tx, &instruction, &tracker).expect("swap emitted");

        assert_eq!(swap.amm, pumpfun::amm::PROGRAM_ID.to_vec());
        assert_eq!(swap.amm_pool, pool.to_vec(), "pool from tracker should populate amm_pool");
        assert_eq!(swap.input_amount, 1_000);
        assert_eq!(swap.output_amount, 900);
    }

    #[test]
    fn unknown_routed_amm_emits_empty_pool() {
        // No tracker entries → Jupiter event for an unknown AMM emits an empty
        // pool (current behaviour, just no longer hardcoded at the call site).
        let tracker = Tracker::new();
        let unknown_amm = [42u8; 32];
        let event_data = jup_swap_event_data(unknown_amm, [1u8; 32], 1, [2u8; 32], 1);
        let jup_tx = make_tx(jupiter::v6::PROGRAM_ID, &[], event_data);

        let instruction = jup_tx.walk_instructions().next().unwrap();
        let swap = decode_instruction(&jup_tx, &instruction, &tracker).expect("swap emitted");

        assert_eq!(swap.amm, unknown_amm.to_vec());
        assert!(swap.amm_pool.is_empty());
    }

    #[test]
    fn non_jupiter_program_yields_nothing() {
        let tracker = Tracker::new();
        let some_program = [99u8; 32];
        let tx = make_tx(some_program, &[[1u8; 32]], vec![0u8; 128]);
        let instruction = tx.walk_instructions().next().unwrap();
        assert!(decode_instruction(&tx, &instruction, &tracker).is_none());
    }
}
