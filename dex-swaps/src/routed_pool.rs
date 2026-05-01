//! Pool-account lookup for AMMs reached via Jupiter CPI.
//!
//! Jupiter v4/v6 Swap events carry the routed AMM program but not the pool
//! account. As we walk a transaction's instructions, [Tracker::observe] feeds
//! each non-Jupiter ix to the per-AMM `extract_pool` helper and remembers the
//! pool keyed by program id; Jupiter handlers later look it up via
//! [Tracker::lookup] when emitting their swap row.
//!
//! The dispatch list mirrors which AMMs have per-AMM pool decoders in this
//! crate. Add new AMMs as their decoders land.
//!
//! ## Per-program latest-wins
//!
//! `Tracker` stores a single pool per program id; `observe` overwrites prior
//! observations. This is correct for **Jupiter v6**: each Swap event self-CPI
//! is emitted immediately after the routed AMM's inner CPI in DFS walk order,
//! so by the time the v6 handler does `lookup` the latest pool for `event.amm`
//! is the right one — even across multi-hop routes that revisit the same AMM
//! program (e.g. Raydium → Orca → Raydium).
//!
//! **Jupiter v4** is more fragile: its Swap log lines are processed in a
//! second pass over `log_messages` after the walk completes, so a direct
//! same-program swap occurring later in the same tx than a v4-routed CPI can
//! overwrite the entry the v4 row should reference, causing misattribution.
//! Jupiter v4 traffic is two orders of magnitude smaller than v6 and rarely
//! interleaves with same-AMM direct activity, so this gap is documented but
//! not yet fixed; the proper fix is per-instruction history rather than a
//! per-program latest-wins map.

use std::collections::HashMap;

use substreams_solana::block_view::InstructionView;

pub(crate) struct Tracker {
    pools: HashMap<[u8; 32], Vec<u8>>,
}

impl Tracker {
    pub(crate) fn new() -> Self {
        Self { pools: HashMap::new() }
    }

    pub(crate) fn observe(&mut self, ix: &InstructionView) {
        let Ok(program_id) = TryInto::<[u8; 32]>::try_into(ix.program_id().0.as_slice()) else {
            return;
        };
        if let Some(pool) = dispatch(ix) {
            self.pools.insert(program_id, pool);
        }
    }

    pub(crate) fn lookup(&self, program_id: &[u8; 32]) -> Option<&Vec<u8>> {
        self.pools.get(program_id)
    }
}

fn dispatch(ix: &InstructionView) -> Option<Vec<u8>> {
    crate::pumpfun_amm::extract_pool(ix)
        .or_else(|| crate::raydium_amm_v4::extract_pool(ix))
        .or_else(|| crate::raydium_clmm::extract_pool(ix))
        .or_else(|| crate::raydium_cpmm::extract_pool(ix))
        .or_else(|| crate::orca_whirlpool::extract_pool(ix))
        .or_else(|| crate::meteora_dlmm::extract_pool(ix))
        .or_else(|| crate::pumpfun::extract_pool(ix))
        .or_else(|| crate::raydium_launchpad::extract_pool(ix))
}

#[cfg(test)]
pub(crate) mod test_fixture {
    //! Helpers for building minimal `pb::ConfirmedTransaction` fixtures so
    //! per-AMM `extract_pool` functions can be exercised in unit tests.
    //!
    //! A fixture is a single compiled instruction with `program_id` at index 0
    //! of the message account_keys, followed by the supplied `accounts` (each
    //! gets its own account_keys slot). The instruction's `accounts` list
    //! references those by index.
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, Transaction,
        TransactionStatusMeta,
    };

    /// Fee payer placeholder — `account_keys[0]` in real Solana txs is the
    /// fee payer, and `common::solana::get_fee_payer` reads index 0. Mirror
    /// that here so any future test that asserts on `user` doesn't get a
    /// program id back.
    pub(crate) const FEE_PAYER: [u8; 32] = [0xfe; 32];

    pub(crate) fn make_tx(program: [u8; 32], accounts: &[[u8; 32]], data: Vec<u8>) -> ConfirmedTransaction {
        let mut keys: Vec<Vec<u8>> = vec![FEE_PAYER.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8); // shift past fee_payer (0) and program (1)
        }
        ConfirmedTransaction {
            transaction: Some(Transaction {
                signatures: vec![vec![0u8; 64]],
                message: Some(Message {
                    header: Some(MessageHeader {
                        num_required_signatures: 1,
                        num_readonly_signed_accounts: 0,
                        num_readonly_unsigned_accounts: 0,
                    }),
                    account_keys: keys,
                    recent_blockhash: vec![0u8; 32],
                    instructions: vec![CompiledInstruction {
                        program_id_index: 1,
                        accounts: acc_idx,
                        data,
                    }],
                    versioned: false,
                    address_table_lookups: vec![],
                }),
            }),
            meta: Some(TransactionStatusMeta::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana_idls::{pumpfun, raydium};

    /// PumpSwap (`pumpfun::amm`) Buy discriminator. Mirrors the private
    /// constant in `substreams_solana_idls::pumpfun::amm::instructions`.
    const PUMPFUN_AMM_BUY: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

    #[test]
    fn tracker_observe_records_pool_for_known_amm() {
        let pool = [9u8; 32];
        let mints_and_filler = [[2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], [6u8; 32]];
        let mut accounts = vec![pool];
        accounts.extend_from_slice(&mints_and_filler);

        // PumpSwap Buy: 8-byte discriminator + 2 u64s (base_amount_out, max_quote_amount_in).
        let mut data = PUMPFUN_AMM_BUY.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes());
        data.extend_from_slice(&50_000u64.to_le_bytes());

        let tx = test_fixture::make_tx(pumpfun::amm::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }

        assert_eq!(tracker.lookup(&pumpfun::amm::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_skips_unknown_program() {
        let unknown_program = [42u8; 32];
        let tx = test_fixture::make_tx(unknown_program, &[[1u8; 32], [2u8; 32]], vec![0u8; 8]);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert!(tracker.lookup(&unknown_program).is_none());
    }

    #[test]
    fn tracker_lookup_missing_returns_none() {
        let tracker = Tracker::new();
        assert!(tracker.lookup(&[7u8; 32]).is_none());
    }

    /// Build a long account list with `pool` placed at `pool_index`. Fills
    /// every other slot with deterministic non-pool bytes so IDL-typed account
    /// extractors can satisfy their `get_req` indices.
    fn accounts_with_pool(pool: [u8; 32], pool_index: usize, total: usize) -> Vec<[u8; 32]> {
        let mut accounts: Vec<[u8; 32]> = (0..total).map(|i| [(i as u8).wrapping_add(50); 32]).collect();
        accounts[pool_index] = pool;
        accounts
    }

    #[test]
    fn tracker_observe_handles_pumpfun_amm_buy_exact_quote_in() {
        // PumpSwap BuyExactQuoteIn: this is the variant Jupiter routes to,
        // not the default Buy. Pool index is the same (accounts[0]).
        const PUMPFUN_AMM_BUY_EXACT_QUOTE_IN: [u8; 8] = [198, 46, 21, 82, 180, 217, 232, 112];
        let pool = [27u8; 32];
        let mints_and_filler = [[2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], [6u8; 32]];
        let mut accounts = vec![pool];
        accounts.extend_from_slice(&mints_and_filler);
        let mut data = PUMPFUN_AMM_BUY_EXACT_QUOTE_IN.to_vec();
        // Real on-chain payload is 16 bytes (two u64s: quote_amount_in, min_base_amount_out).
        data.extend_from_slice(&100_000u64.to_le_bytes());
        data.extend_from_slice(&50_000u64.to_le_bytes());

        let tx = test_fixture::make_tx(pumpfun::amm::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&pumpfun::amm::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_pumpfun_bonding_curve() {
        // Pumpfun bonding curve: pool at accounts[3] on Buy/Sell.
        const PUMPFUN_BUY: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
        let pool = [21u8; 32];
        let accounts = accounts_with_pool(pool, 3, 12);
        let mut data = PUMPFUN_BUY.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes());
        data.extend_from_slice(&50_000u64.to_le_bytes());
        let tx = test_fixture::make_tx(pumpfun::bonding_curve::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&pumpfun::bonding_curve::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_meteora_dlmm() {
        // Meteora DLMM Swap: lb_pair at accounts[0] (per SwapAccounts IDL).
        const METEORA_DLMM_SWAP: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
        use substreams_solana_idls::meteora;
        let pool = [22u8; 32];
        let accounts = accounts_with_pool(pool, 0, 16);
        let mut data = METEORA_DLMM_SWAP.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes());
        data.extend_from_slice(&50_000u64.to_le_bytes());
        let tx = test_fixture::make_tx(meteora::dlmm::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&meteora::dlmm::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_raydium_clmm() {
        // Raydium CLMM Swap: pool_state at accounts[2].
        const RAYDIUM_CLMM_SWAP: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
        let pool = [23u8; 32];
        let accounts = accounts_with_pool(pool, 2, 14);
        let mut data = RAYDIUM_CLMM_SWAP.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes()); // amount
        data.extend_from_slice(&50_000u64.to_le_bytes());  // other_amount_threshold
        data.extend_from_slice(&0u128.to_le_bytes());      // sqrt_price_limit_x64
        data.push(1);                                      // is_base_input
        let tx = test_fixture::make_tx(raydium::clmm::v3::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&raydium::clmm::v3::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_raydium_cpmm() {
        // Raydium CPMM SwapBaseInput: pool_state at accounts[3].
        const RAYDIUM_CPMM_SWAP_BASE_INPUT: [u8; 8] = [143, 190, 90, 218, 196, 30, 51, 222];
        let pool = [24u8; 32];
        let accounts = accounts_with_pool(pool, 3, 14);
        let mut data = RAYDIUM_CPMM_SWAP_BASE_INPUT.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes()); // amount_in
        data.extend_from_slice(&50_000u64.to_le_bytes());  // minimum_amount_out
        let tx = test_fixture::make_tx(raydium::cpmm::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&raydium::cpmm::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_orca_whirlpool() {
        // Orca Whirlpool Swap: whirlpool at accounts[2].
        const ORCA_SWAP: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
        use substreams_solana_idls::orca;
        let pool = [25u8; 32];
        let accounts = accounts_with_pool(pool, 2, 14);
        let mut data = ORCA_SWAP.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes()); // amount
        data.extend_from_slice(&50_000u64.to_le_bytes());  // other_amount_threshold
        data.extend_from_slice(&0u128.to_le_bytes());      // sqrt_price_limit
        data.push(1);                                      // amount_specified_is_input
        data.push(1);                                      // a_to_b
        let tx = test_fixture::make_tx(orca::whirlpool::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&orca::whirlpool::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_raydium_launchpad() {
        // Raydium Launchpad BuyExactIn: pool_state index per IDL.
        const LAUNCHPAD_BUY_EXACT_IN: [u8; 8] = [250, 234, 13, 123, 213, 156, 19, 236];
        let pool = [26u8; 32];
        // Try a generous account count + place pool at typical pool_state index;
        // adjust if assertion fails.
        let accounts = accounts_with_pool(pool, 4, 18);
        let mut data = LAUNCHPAD_BUY_EXACT_IN.to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes()); // amount_in
        data.extend_from_slice(&50_000u64.to_le_bytes());  // minimum_amount_out
        data.extend_from_slice(&0u64.to_le_bytes());       // share_fee_rate
        let tx = test_fixture::make_tx(raydium::launchpad::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&raydium::launchpad::PROGRAM_ID), Some(&pool.to_vec()));
    }

    #[test]
    fn tracker_observe_handles_raydium_amm_v4() {
        // Raydium AMM v4 SwapBaseIn: program_id + accounts[1] = pool.
        // Account count chooses the offset (without target_orders → 17 accounts).
        let pool = [11u8; 32];
        let mut accounts: Vec<[u8; 32]> = (0..17).map(|i| [i as u8 + 100; 32]).collect();
        accounts[1] = pool;

        // SwapBaseIn discriminator is a single byte (9), then two u64s.
        // raydium::amm::v4::instructions reads u8 prefix.
        let mut data = vec![9u8];
        data.extend_from_slice(&100_000u64.to_le_bytes());
        data.extend_from_slice(&50_000u64.to_le_bytes());

        let tx = test_fixture::make_tx(raydium::amm::v4::PROGRAM_ID, &accounts, data);
        let mut tracker = Tracker::new();
        for ix in tx.walk_instructions() {
            tracker.observe(&ix);
        }
        assert_eq!(tracker.lookup(&raydium::amm::v4::PROGRAM_ID), Some(&pool.to_vec()));
    }
}
