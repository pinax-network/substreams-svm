use std::collections::VecDeque;

use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::okx;

#[derive(Clone)]
struct SwapContext {
    stack_height: u32,
    user: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
    route_index: usize,
    route_count: usize,
}

struct RoutePool {
    dex: okx::v2::instructions::Dex,
    amount_in: u64,
    amm_pool: Vec<u8>,
}

struct PendingEvent {
    event: okx::v2::events::SwapEvent,
    route: Option<RoutePool>,
}

pub(crate) struct State {
    contexts: VecDeque<SwapContext>,
    current: Option<SwapContext>,
    route_pools: VecDeque<RoutePool>,
    previous_log: Option<String>,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            contexts: VecDeque::new(),
            current: None,
            route_pools: VecDeque::new(),
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

        let route_count = swap
            .args
            .routes
            .iter()
            .flat_map(|route| route.iter())
            .map(|route| route.dexes.len())
            .sum::<usize>()
            .max(1);

        self.contexts.push_back(SwapContext {
            stack_height: ix.stack_height(),
            user: accounts.payer.to_bytes().to_vec(),
            input_mint: accounts.source_mint.to_bytes().to_vec(),
            output_mint: accounts.destination_mint.to_bytes().to_vec(),
            route_index: 0,
            route_count,
        });
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        if let Some(previous_log) = self.previous_log.as_deref() {
            if let Ok(okx::v2::logs::OkxV2Log::RouteAmmPool(route)) = okx::v2::logs::unpack_route_amm_pool(previous_log, log_message) {
                self.ensure_context()?;
                self.route_pools.push_back(RoutePool {
                    dex: route.dex,
                    amount_in: route.amount_in,
                    amm_pool: route.amm_pool.to_bytes().to_vec(),
                });
                self.previous_log = Some(log_message.to_string());
                return None;
            }
        }

        if let Ok(okx::v2::logs::OkxV2Log::Swap(event)) = okx::v2::logs::unpack(log_message) {
            self.ensure_context()?;
            let route = self.pop_route_for(&event);
            self.previous_log = Some(log_message.to_string());
            return Some(self.build_swap(PendingEvent { event, route }));
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

    fn pop_route_for(&mut self, event: &okx::v2::events::SwapEvent) -> Option<RoutePool> {
        let index = self
            .route_pools
            .iter()
            .position(|route| route.dex == event.dex && route.amount_in == event.amount_in)
            .unwrap_or(0);
        self.route_pools.remove(index)
    }

    fn build_swap(&mut self, pending: PendingEvent) -> pb::Swap {
        let context = self.current.as_mut().expect("OKX context exists while building swaps");
        let route_index = context.route_index;
        context.route_index += 1;

        let input_mint = if route_index == 0 { context.input_mint.clone() } else { Vec::new() };
        let output_mint = if route_index + 1 == context.route_count {
            context.output_mint.clone()
        } else {
            Vec::new()
        };

        let swap = pb::Swap {
            protocol: pb::Protocol::OkxDex as i32,
            program_id: okx::v2::PROGRAM_ID.to_vec(),
            stack_height: context.stack_height,
            amm: okx::v2::PROGRAM_ID.to_vec(),
            amm_pool: pending.route.map(|route| route.amm_pool).unwrap_or_default(),
            user: context.user.clone(),
            input_mint,
            input_amount: pending.event.amount_in,
            output_mint,
            output_amount: pending.event.amount_out,
        };

        if context.route_index >= context.route_count {
            self.current = None;
            self.route_pools.clear();
        }

        swap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use substreams_solana::base58;

    fn key(value: &str) -> [u8; 32] {
        base58::decode(value).unwrap().try_into().unwrap()
    }

    #[test]
    fn emits_swap_from_tessera_logs_with_pool_and_mints() {
        let payer = key("BpEBKwah2sGwa9zNtdoUgdmEDdms8L2NkFLfXShrz1ac");
        let source = key("6KiH81VTVqNGqwkDgtk2qV7rRjWfSXrMaRD45KW5VG8K");
        let destination = key("4NYrbAm1jfjgnV9JUTQbZ5VJoXYGHGKZcZqGLupCwJDU");
        let source_mint = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let destination_mint = key("So11111111111111111111111111111111111111112");
        let source_sa = key("HjkGLCPnsMr4yP2Tmi1Uj7gV7Y2xDj2Npn9kYfVBYr2s");
        let destination_sa = key("2rikd7tzPbmowhUJzPNVtX7fuUGcnBa8jqJnx6HbtHeE");
        let pool = key("FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n");
        let data = substreams::hex!("f0e02621b01ff1afbf775f3b05000000db7706a03a000000db7706a03a00000001000000bf775f3b050000000100000001000000010000003f0100000064000000000000000000000000000000000000").to_vec();
        let tx = make_tx(
            okx::v2::PROGRAM_ID,
            &[
                payer,
                source,
                destination,
                source_mint,
                destination_mint,
                [6; 32],
                [7; 32],
                [8; 32],
                source_sa,
                destination_sa,
            ],
            data,
        );
        let mut state = State::new();
        let instruction = tx.walk_instructions().next().unwrap();
        state.handle_instruction(&instruction);

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::Tessera amount_in: 22470948799, offset: 0",
            "Program log: FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH invoke [2]",
            "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH success",
            "Program log: SwapEvent { dex: Tessera, amount_in: 22470948799, amount_out: 251803859108 }",
            "Program log: HjkGLCPnsMr4yP2Tmi1Uj7gV7Y2xDj2Npn9kYfVBYr2s",
            "Program log: 2rikd7tzPbmowhUJzPNVtX7fuUGcnBa8jqJnx6HbtHeE",
        ];

        let swap = logs.iter().find_map(|log| state.handle_log(log)).expect("OKX swap");

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
    fn emits_multihop_swaps_from_sanctum_and_alphaq_logs() {
        let payer = key("GMBvzdJ97CykmBdcNuxR6GTFy15Rn3jsY9vz1nVbAPGk");
        let source = [2; 32];
        let destination = [3; 32];
        let jitosol_mint = key("J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn");
        let wsol_mint = key("So11111111111111111111111111111111111111112");
        let first_source = key("D5wjgMadAHGstes5ZTCuw8XnGhxiQdsiMo3D44bei3tu");
        let bridge_account = key("5eShYY2cJgghtjnzWMZAmfEMzwg6NBjdPrvP95ycKcsw");
        let final_destination = key("2rikd7tzPbmowhUJzPNVtX7fuUGcnBa8jqJnx6HbtHeE");
        let sanctum_pool = key("AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW");
        let alphaq_pool = key("C2GdMFGp2vSZHnU76pH2ukEWxuhoJBuaA54Ftzcvv4z5");
        let data = substreams::hex!("f0e02621b01ff1af7cc51c7f010000003bcc0fe901000000bfcc2be401000000010000007cc51c7f01000000010000000200000001000000150100000064010000005301000000640000000000000000000000000000").to_vec();
        let tx = make_tx(
            okx::v2::PROGRAM_ID,
            &[
                payer,
                source,
                destination,
                jitosol_mint,
                wsol_mint,
                [6; 32],
                [7; 32],
                [8; 32],
                first_source,
                bridge_account,
                final_destination,
            ],
            data,
        );
        let mut state = State::new();
        let instruction = tx.walk_instructions().next().unwrap();
        state.handle_instruction(&instruction);

        let logs = [
            "Program 6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma invoke [1]",
            "Program log: Dex::SanctumSwapWithoutWsol amount_in: 6427559292, offset: 0",
            "Program log: AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW",
            "Program 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx invoke [2]",
            "Program 5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx success",
            "Program log: SwapEvent { dex: SanctumNonWsolSwap, amount_in: 6427559292, amount_out: 6884273882 }",
            "Program log: D5wjgMadAHGstes5ZTCuw8XnGhxiQdsiMo3D44bei3tu",
            "Program log: 5eShYY2cJgghtjnzWMZAmfEMzwg6NBjdPrvP95ycKcsw",
            "Program log: Dex::AlphaQ amount_in: 6884273882, offset: 25",
            "Program log: C2GdMFGp2vSZHnU76pH2ukEWxuhoJBuaA54Ftzcvv4z5",
            "Program ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA invoke [2]",
            "Program ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA success",
            "Program log: SwapEvent { dex: AlphaQ, amount_in: 6884273882, amount_out: 8156649077 }",
            "Program log: 5eShYY2cJgghtjnzWMZAmfEMzwg6NBjdPrvP95ycKcsw",
            "Program log: 2rikd7tzPbmowhUJzPNVtX7fuUGcnBa8jqJnx6HbtHeE",
        ];
        let swaps = logs.iter().filter_map(|log| state.handle_log(log)).collect::<Vec<_>>();

        assert_eq!(swaps.len(), 2);
        assert_eq!(swaps[0].amm, okx::v2::PROGRAM_ID.to_vec());
        assert_eq!(swaps[0].amm_pool, sanctum_pool.to_vec());
        assert_eq!(swaps[0].input_mint, jitosol_mint.to_vec());
        assert_eq!(swaps[0].input_amount, 6_427_559_292);
        assert_eq!(swaps[0].output_mint, Vec::<u8>::new());
        assert_eq!(swaps[0].output_amount, 6_884_273_882);
        assert_eq!(swaps[1].amm, okx::v2::PROGRAM_ID.to_vec());
        assert_eq!(swaps[1].amm_pool, alphaq_pool.to_vec());
        assert_eq!(swaps[1].input_mint, Vec::<u8>::new());
        assert_eq!(swaps[1].input_amount, 6_884_273_882);
        assert_eq!(swaps[1].output_mint, wsol_mint.to_vec());
        assert_eq!(swaps[1].output_amount, 8_156_649_077);
    }
}
