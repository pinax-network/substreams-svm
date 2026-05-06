//! OKX Dex v2 router swap normalization.
//!
//! OKX's `SwapV3` instruction is a top-level router that CPI-calls one or
//! more underlying DEX programs to fulfil a `source_mint → destination_mint`
//! trade. The router emits one `SwapEvent` log line per underlying CPI,
//! preceded by a two-line preamble (`Dex::<variant> amount_in: <n>, offset:
//! <m>` then a base58 pool pubkey) that pins each event to the AMM pool the
//! router routed through.
//!
//! We emit ONE normalized swap row per `SwapV3` instruction with:
//! - `input_mint` / `output_mint`: from the SwapV3 accounts (always known).
//! - `input_amount`: from `SwapV3.args.amount_in` — the user-supplied amount.
//! - `output_amount`: the last `SwapEvent.amount_out` observed for this
//!   instruction (final hop's output for linear routes).
//! - `amm_pool`: the route preamble's pool pubkey when the route is a single
//!   hop. Multi-hop routes emit an empty `amm_pool` because no single
//!   underlying pool represents a routed trade — per-pool detail is captured
//!   by the per-DEX decoders for the same transaction.
//!
//! Top-level `Program <OKX_PROGRAM_ID> invoke` log lines mark instruction
//! boundaries and reset any in-flight state, so multiple OKX swaps in the
//! same tx cannot leak context across each other.

use std::collections::VecDeque;

use common::solana::{is_invoke, parse_program_id};
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::okx;

struct SwapContext {
    stack_height: u32,
    user: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
    input_amount: u64,
    expected_events: usize,
    received_events: usize,
    last_event_amount_out: u64,
    first_event_pool: Option<Vec<u8>>,
}

pub(crate) struct State {
    contexts: VecDeque<SwapContext>,
    current: Option<SwapContext>,
    pending_route_pool: Option<Vec<u8>>,
    previous_log: Option<String>,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            contexts: VecDeque::new(),
            current: None,
            pending_route_pool: None,
            previous_log: None,
        }
    }

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView) {
        if ix.program_id().0 != &okx::v2::PROGRAM_ID {
            return;
        }
        let Ok(okx::v2::instructions::OkxV2Instruction::SwapV3(swap)) = okx::v2::instructions::unpack(ix.data()) else {
            return;
        };
        let Ok(accounts) = okx::v2::accounts::get_swap_v3_accounts(ix) else {
            return;
        };

        let expected_events = swap
            .args
            .routes
            .iter()
            .flat_map(|chain| chain.iter())
            .map(|route| route.dexes.len())
            .sum::<usize>()
            .max(1);

        self.contexts.push_back(SwapContext {
            stack_height: ix.stack_height(),
            user: accounts.payer.to_bytes().to_vec(),
            input_mint: accounts.source_mint.to_bytes().to_vec(),
            output_mint: accounts.destination_mint.to_bytes().to_vec(),
            input_amount: swap.args.amount_in,
            expected_events,
            received_events: 0,
            last_event_amount_out: 0,
            first_event_pool: None,
        });
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        // Skip per-log parsing entirely when no OKX SwapV3 is in scope for
        // this tx. Avoids a `String` allocation per log line in the common
        // case where the tx contains zero OKX swaps.
        if self.current.is_none() && self.contexts.is_empty() {
            return None;
        }

        // Top-level `Program <OKX> invoke [N]` marks the start of a new
        // SwapV3 invocation. Drop any in-flight state from a prior invocation
        // that didn't reach its expected event count (e.g. internal program
        // logic shortcut a hop) so we never leak across instructions.
        if is_okx_invoke(log_message) {
            self.current = None;
            self.pending_route_pool = None;
            self.previous_log = None;
            return None;
        }

        // Two-line route preamble: "Program log: Dex::X amount_in: ..., offset: ..."
        // followed by "Program log: <pool_pubkey>".
        if let Some(prev) = self.previous_log.as_deref() {
            if let Ok(okx::v2::logs::OkxV2Log::RouteAmmPool(route)) = okx::v2::logs::unpack_route_amm_pool(prev, log_message) {
                if self.ensure_context().is_some() {
                    self.pending_route_pool = Some(route.amm_pool.to_bytes().to_vec());
                }
                self.previous_log = Some(log_message.to_string());
                return None;
            }
        }

        // Router-emitted SwapEvent log marks a hop's completion.
        if let Ok(okx::v2::logs::OkxV2Log::Swap(event)) = okx::v2::logs::unpack(log_message) {
            self.previous_log = Some(log_message.to_string());
            if self.ensure_context().is_none() {
                return None;
            }
            return self.consume_event(&event);
        }

        self.previous_log = Some(log_message.to_string());
        None
    }

    fn ensure_context(&mut self) -> Option<()> {
        if self.current.is_none() {
            self.current = self.contexts.pop_front();
        }
        self.current.as_ref()?;
        Some(())
    }

    fn consume_event(&mut self, event: &okx::v2::events::SwapEvent) -> Option<pb::Swap> {
        let pending_pool = self.pending_route_pool.take();
        let ctx = self.current.as_mut().expect("ensure_context returned Some");

        // Capture the AMM pool only for single-hop routes; for multi-hop
        // routes the first hop's pool is not representative of the user's
        // overall trade, so we leave it empty.
        if ctx.received_events == 0 && ctx.expected_events == 1 {
            ctx.first_event_pool = pending_pool;
        }
        ctx.received_events += 1;
        ctx.last_event_amount_out = event.amount_out;

        if ctx.received_events >= ctx.expected_events {
            return self.emit_summary();
        }
        None
    }

    fn emit_summary(&mut self) -> Option<pb::Swap> {
        let ctx = self.current.take()?;
        self.pending_route_pool = None;

        // Defensive: the SwapV3 accounts struct guarantees these are set, but
        // skip if anything upstream produced an empty mint to keep
        // `state_ohlc_prices` clean.
        if ctx.input_mint.is_empty() || ctx.output_mint.is_empty() {
            return None;
        }

        Some(pb::Swap {
            protocol: pb::Protocol::OkxDex as i32,
            program_id: okx::v2::PROGRAM_ID.to_vec(),
            stack_height: ctx.stack_height,
            amm: okx::v2::PROGRAM_ID.to_vec(),
            amm_pool: ctx.first_event_pool.unwrap_or_default(),
            user: ctx.user,
            input_mint: ctx.input_mint,
            input_amount: ctx.input_amount,
            output_mint: ctx.output_mint,
            output_amount: ctx.last_event_amount_out,
        })
    }
}

fn is_okx_invoke(log: &str) -> bool {
    is_invoke(log) && parse_program_id(log).map_or(false, |id| id == okx::v2::PROGRAM_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use substreams_solana::base58;

    /// Borsh tag for the `Tessera` `Dex` variant in `okx::v2::instructions::Dex`.
    const DEX_SANCTUM_NON_WSOL_SWAP: u8 = 21;
    const DEX_TESSERA: u8 = 63;
    const DEX_ALPHA_Q: u8 = 83;

    fn key(value: &str) -> [u8; 32] {
        base58::decode(value).unwrap().try_into().unwrap()
    }

    /// Hand-rolled Borsh payload for a `SwapV3` instruction. `hop_dexes` lists
    /// the variant tags for a single linear route (no parallel splits within
    /// a hop). `amount_in` is reused for `expect_amount_out` and `min_return`
    /// since handle_instruction only reads `args.amount_in` from the payload.
    fn build_swap_v3_payload(amount_in: u64, hop_dexes: &[u8]) -> Vec<u8> {
        let mut data = vec![0xf0, 0xe0, 0x26, 0x21, 0xb0, 0x1f, 0xf1, 0xaf]; // SWAP_V3 disc
        data.extend_from_slice(&amount_in.to_le_bytes()); // amount_in
        data.extend_from_slice(&amount_in.to_le_bytes()); // expect_amount_out
        data.extend_from_slice(&amount_in.to_le_bytes()); // min_return
        data.extend_from_slice(&1u32.to_le_bytes()); // amounts.len
        data.extend_from_slice(&amount_in.to_le_bytes()); // amounts[0]
        data.extend_from_slice(&1u32.to_le_bytes()); // routes.len
        data.extend_from_slice(&(hop_dexes.len() as u32).to_le_bytes()); // routes[0].len
        for &dex in hop_dexes {
            data.extend_from_slice(&1u32.to_le_bytes()); // dexes.len = 1
            data.push(dex); // dex variant tag
            data.extend_from_slice(&1u32.to_le_bytes()); // weights.len = 1
            data.push(100); // weights[0]
        }
        data.extend_from_slice(&0u32.to_le_bytes()); // commission_info
        data.extend_from_slice(&0u16.to_le_bytes()); // platform_fee_rate
        data.extend_from_slice(&0u64.to_le_bytes()); // order_id
        data
    }

    #[test]
    fn single_hop_emits_summary_with_pool() {
        let payer = key("BpEBKwah2sGwa9zNtdoUgdmEDdms8L2NkFLfXShrz1ac");
        let source = key("6KiH81VTVqNGqwkDgtk2qV7rRjWfSXrMaRD45KW5VG8K");
        let destination = key("4NYrbAm1jfjgnV9JUTQbZ5VJoXYGHGKZcZqGLupCwJDU");
        let source_mint = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let destination_mint = key("So11111111111111111111111111111111111111112");
        let pool = key("FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n");
        let tx = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer, source, destination, source_mint, destination_mint, [6; 32], [7; 32], [8; 32]],
            build_swap_v3_payload(22_470_948_799, &[DEX_TESSERA]),
        );
        let mut state = State::new();
        state.handle_instruction(&tx.walk_instructions().next().unwrap());

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::Tessera amount_in: 22470948799, offset: 0",
            "Program log: FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH invoke [2]",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH success",
            "Program log: SwapEvent { dex: Tessera, amount_in: 22470948799, amount_out: 251803859108 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();

        assert_eq!(swaps.len(), 1);
        let swap = &swaps[0];
        assert_eq!(swap.protocol, pb::Protocol::OkxDex as i32);
        assert_eq!(swap.program_id, okx::v2::PROGRAM_ID.to_vec());
        assert_eq!(swap.amm, okx::v2::PROGRAM_ID.to_vec());
        assert_eq!(swap.amm_pool, pool.to_vec());
        assert_eq!(swap.user, payer.to_vec());
        assert_eq!(swap.input_mint, source_mint.to_vec());
        assert_eq!(swap.input_amount, 22_470_948_799);
        assert_eq!(swap.output_mint, destination_mint.to_vec());
        assert_eq!(swap.output_amount, 251_803_859_108);
    }

    #[test]
    fn multi_hop_emits_one_summary_row_with_empty_pool() {
        let payer = key("GMBvzdJ97CykmBdcNuxR6GTFy15Rn3jsY9vz1nVbAPGk");
        let jitosol_mint = key("J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn");
        let wsol_mint = key("So11111111111111111111111111111111111111112");
        let tx = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer, [2; 32], [3; 32], jitosol_mint, wsol_mint, [6; 32], [7; 32], [8; 32]],
            build_swap_v3_payload(6_427_559_292, &[DEX_SANCTUM_NON_WSOL_SWAP, DEX_ALPHA_Q]),
        );
        let mut state = State::new();
        state.handle_instruction(&tx.walk_instructions().next().unwrap());

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::SanctumSwapWithoutWsol amount_in: 6427559292, offset: 0",
            "Program log: AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW",
            "Program 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx invoke [2]",
            "Program 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx success",
            "Program log: SwapEvent { dex: SanctumNonWsolSwap, amount_in: 6427559292, amount_out: 6884273882 }",
            "Program log: Dex::AlphaQ amount_in: 6884273882, offset: 25",
            "Program log: C2GdMFGp2vSZHnU76pH2ukEWxuhoJBuaA54Ftzcvv4z5",
            "Program ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA invoke [2]",
            "Program ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA success",
            "Program log: SwapEvent { dex: AlphaQ, amount_in: 6884273882, amount_out: 8156649077 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();

        assert_eq!(swaps.len(), 1, "multi-hop emits one summary row, not per-hop rows with empty mints");
        let swap = &swaps[0];
        assert_eq!(swap.user, payer.to_vec());
        assert_eq!(swap.input_mint, jitosol_mint.to_vec());
        assert_eq!(swap.output_mint, wsol_mint.to_vec());
        assert_eq!(swap.input_amount, 6_427_559_292, "input from args.amount_in");
        assert_eq!(swap.output_amount, 8_156_649_077, "output from final hop's amount_out");
        assert!(swap.amm_pool.is_empty(), "no single pool represents a multi-hop route");
    }

    #[test]
    fn boundary_log_drops_partial_first_swap_and_second_attributes_cleanly() {
        // Two OKX SwapV3 instructions in the same tx. The first declares 2
        // expected hops but only 1 SwapEvent fires before the second
        // SwapV3's `Program ... invoke [1]` boundary. Without boundary
        // detection, the first context would stay current and the second
        // SwapV3's SwapEvent would be misattributed to it.
        let payer1 = key("GMBvzdJ97CykmBdcNuxR6GTFy15Rn3jsY9vz1nVbAPGk");
        let src_mint1 = key("J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn");
        let dst_mint1 = key("So11111111111111111111111111111111111111112");
        let payer2 = key("BpEBKwah2sGwa9zNtdoUgdmEDdms8L2NkFLfXShrz1ac");
        let src_mint2 = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let dst_mint2 = key("4NYrbAm1jfjgnV9JUTQbZ5VJoXYGHGKZcZqGLupCwJDU");
        let pool2 = key("FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n");
        let tx1 = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer1, [10; 32], [11; 32], src_mint1, dst_mint1, [12; 32], [13; 32], [14; 32]],
            build_swap_v3_payload(1_000_000, &[DEX_SANCTUM_NON_WSOL_SWAP, DEX_ALPHA_Q]),
        );
        let tx2 = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer2, [20; 32], [21; 32], src_mint2, dst_mint2, [22; 32], [23; 32], [24; 32]],
            build_swap_v3_payload(2_000_000, &[DEX_TESSERA]),
        );
        let mut state = State::new();
        state.handle_instruction(&tx1.walk_instructions().next().unwrap());
        state.handle_instruction(&tx2.walk_instructions().next().unwrap());

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::SanctumSwapWithoutWsol amount_in: 1000000, offset: 0",
            "Program log: AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW",
            "Program 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx invoke [2]",
            "Program 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx success",
            "Program log: SwapEvent { dex: SanctumNonWsolSwap, amount_in: 1000000, amount_out: 1500000 }",
            // tx1's second hop never fires; tx2 starts here.
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::Tessera amount_in: 2000000, offset: 0",
            "Program log: FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH invoke [2]",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH success",
            "Program log: SwapEvent { dex: Tessera, amount_in: 2000000, amount_out: 3000000 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();

        assert_eq!(swaps.len(), 1, "tx1 partial dropped, only tx2 emits");
        let swap = &swaps[0];
        assert_eq!(swap.user, payer2.to_vec(), "swap must attribute to tx2's payer, not tx1's");
        assert_eq!(swap.input_mint, src_mint2.to_vec());
        assert_eq!(swap.output_mint, dst_mint2.to_vec());
        assert_eq!(swap.input_amount, 2_000_000);
        assert_eq!(swap.output_amount, 3_000_000);
        assert_eq!(swap.amm_pool, pool2.to_vec());
    }

    #[test]
    fn boundary_log_separates_two_completed_swaps() {
        // Both OKX SwapV3's complete cleanly. Each emits its own row with
        // its own context, with no overlap.
        let payer1 = key("GMBvzdJ97CykmBdcNuxR6GTFy15Rn3jsY9vz1nVbAPGk");
        let src_mint1 = key("J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn");
        let dst_mint1 = key("So11111111111111111111111111111111111111112");
        let pool1 = key("AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW");
        let payer2 = key("BpEBKwah2sGwa9zNtdoUgdmEDdms8L2NkFLfXShrz1ac");
        let src_mint2 = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let dst_mint2 = key("4NYrbAm1jfjgnV9JUTQbZ5VJoXYGHGKZcZqGLupCwJDU");
        let pool2 = key("FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n");
        let tx1 = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer1, [10; 32], [11; 32], src_mint1, dst_mint1, [12; 32], [13; 32], [14; 32]],
            build_swap_v3_payload(1_000_000, &[DEX_SANCTUM_NON_WSOL_SWAP]),
        );
        let tx2 = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer2, [20; 32], [21; 32], src_mint2, dst_mint2, [22; 32], [23; 32], [24; 32]],
            build_swap_v3_payload(2_000_000, &[DEX_TESSERA]),
        );
        let mut state = State::new();
        state.handle_instruction(&tx1.walk_instructions().next().unwrap());
        state.handle_instruction(&tx2.walk_instructions().next().unwrap());

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::SanctumSwapWithoutWsol amount_in: 1000000, offset: 0",
            "Program log: AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW",
            "Program log: SwapEvent { dex: SanctumNonWsolSwap, amount_in: 1000000, amount_out: 1500000 }",
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::Tessera amount_in: 2000000, offset: 0",
            "Program log: FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n",
            "Program log: SwapEvent { dex: Tessera, amount_in: 2000000, amount_out: 3000000 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();

        assert_eq!(swaps.len(), 2);
        assert_eq!(swaps[0].user, payer1.to_vec());
        assert_eq!(swaps[0].input_mint, src_mint1.to_vec());
        assert_eq!(swaps[0].output_mint, dst_mint1.to_vec());
        assert_eq!(swaps[0].input_amount, 1_000_000);
        assert_eq!(swaps[0].output_amount, 1_500_000);
        assert_eq!(swaps[0].amm_pool, pool1.to_vec());
        assert_eq!(swaps[1].user, payer2.to_vec());
        assert_eq!(swaps[1].input_mint, src_mint2.to_vec());
        assert_eq!(swaps[1].output_mint, dst_mint2.to_vec());
        assert_eq!(swaps[1].input_amount, 2_000_000);
        assert_eq!(swaps[1].output_amount, 3_000_000);
        assert_eq!(swaps[1].amm_pool, pool2.to_vec());
    }

    #[test]
    fn swap_event_without_route_preamble_emits_empty_pool_not_a_guess() {
        // Single-hop SwapV3, but the route preamble logs are absent (defensive
        // case — shouldn't happen on chain but we don't want a silent
        // misattribution if it does). Result: emit summary row with empty
        // amm_pool rather than mis-pinning the swap to whatever pool happened
        // to be in a stale buffer.
        let payer = key("BpEBKwah2sGwa9zNtdoUgdmEDdms8L2NkFLfXShrz1ac");
        let source_mint = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let destination_mint = key("So11111111111111111111111111111111111111112");
        let tx = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer, [10; 32], [11; 32], source_mint, destination_mint, [12; 32], [13; 32], [14; 32]],
            build_swap_v3_payload(500, &[DEX_TESSERA]),
        );
        let mut state = State::new();
        state.handle_instruction(&tx.walk_instructions().next().unwrap());

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH invoke [2]",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH success",
            "Program log: SwapEvent { dex: Tessera, amount_in: 500, amount_out: 750 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();

        assert_eq!(swaps.len(), 1);
        assert!(swaps[0].amm_pool.is_empty());
        assert_eq!(swaps[0].input_mint, source_mint.to_vec());
        assert_eq!(swaps[0].output_mint, destination_mint.to_vec());
        assert_eq!(swaps[0].output_amount, 750);
    }

    #[test]
    fn handle_log_short_circuits_when_no_okx_context_is_queued() {
        // No SwapV3 in the tx → handle_log must early-out and not allocate
        // `previous_log` even for log lines that would otherwise parse.
        let mut state = State::new();
        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::Tessera amount_in: 100, offset: 0",
            "Program log: FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n",
            "Program log: SwapEvent { dex: Tessera, amount_in: 100, amount_out: 200 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();
        assert!(swaps.is_empty());
        assert!(state.previous_log.is_none(), "fast-path must skip allocation when nothing is queued");
    }

    #[test]
    fn extra_swap_event_after_summary_does_not_panic_or_corrupt() {
        // Single-hop SwapV3 emits one SwapEvent; we summary-emit. A stray
        // SwapEvent after that (defensive) should not crash and (with no
        // queued context) must not produce a row.
        let payer = key("BpEBKwah2sGwa9zNtdoUgdmEDdms8L2NkFLfXShrz1ac");
        let source_mint = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let destination_mint = key("So11111111111111111111111111111111111111112");
        let tx = make_tx(
            okx::v2::PROGRAM_ID,
            &[payer, [10; 32], [11; 32], source_mint, destination_mint, [12; 32], [13; 32], [14; 32]],
            build_swap_v3_payload(100, &[DEX_TESSERA]),
        );
        let mut state = State::new();
        state.handle_instruction(&tx.walk_instructions().next().unwrap());

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::Tessera amount_in: 100, offset: 0",
            "Program log: FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n",
            "Program log: SwapEvent { dex: Tessera, amount_in: 100, amount_out: 200 }",
            "Program log: SwapEvent { dex: Tessera, amount_in: 999, amount_out: 999 }",
        ];
        let swaps = logs.iter().filter_map(|l| state.handle_log(l)).collect::<Vec<_>>();
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].input_amount, 100);
        assert_eq!(swaps[0].output_amount, 200);
    }
}
