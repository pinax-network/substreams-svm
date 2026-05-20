use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana_idls::darklake;

use crate::logs::{scoped_program_log, ProgramLog};

pub(crate) struct State {
    is_invoked: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self { is_invoked: false }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &darklake::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let data = parse_program_data(log_message)?;
        // Darklake is a commit-reveal AMM: the `Swap` instruction deposits
        // tokens with a commitment hash (intent), and a relayer later calls
        // `Settle` with a zk proof to realise the trade. Cancellation and
        // slashing paths never produce a `SettleEvent`, so matching only
        // `SettleEvent` gives us exactly the realised swaps.
        let event = match darklake::events::unpack_event(data.as_slice()) {
            Ok(darklake::events::DarklakeEvent::Settle(event)) => event,
            _ => return None,
        };

        // `direction` is the borsh-serialised `is_swap_x_to_y: bool`
        // (1 = x → y, 0 = y → x).
        let is_x_to_y = event.direction == 1;
        let (input_mint, output_mint) = if is_x_to_y {
            (event.token_mint_x.to_bytes().to_vec(), event.token_mint_y.to_bytes().to_vec())
        } else {
            (event.token_mint_y.to_bytes().to_vec(), event.token_mint_x.to_bytes().to_vec())
        };

        Some(pb::Swap {
            protocol: pb::Protocol::Darklake as i32,
            program_id: darklake::PROGRAM_ID.to_vec(),
            stack_height: 0,
            amm: darklake::PROGRAM_ID.to_vec(),
            amm_pool: darklake::PROGRAM_ID.to_vec(),
            user: event.trader.to_bytes().to_vec(),
            input_mint,
            input_amount: event.actual_amount_in,
            output_mint,
            output_amount: event.actual_amount_out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // On-chain Darklake Settle event captured from mainnet tx
    // 56BDsUWsCyE4sSAkAzTktZdi2RMwy1eo7TxNdkUKRUkWZL4oBQtjD4bvJwteBtAG9GaHFRi5beHsJtzR6MyFBQke
    // (wSOL → token, direction = 1 = x → y, 10_000_000 → 1_256_182).
    const SETTLE_LOG: &str = "Program data: rFhWSePRzDjbB8sJiZk3v19TqE6qKNp5MtRG2db85Fba1w4g65weItsHywmJmTe/X1OoTqoo2nky1EbZ1vzkVtrXDiDrnB4iAUoDthcAAAAAqGEAAAAAAAAw05cAAAAAAPYqEwAAAAAAgJaYAAAAAABA2EEAAAAAAIgTAAAAAAAAYEsnAAAAAAD2KhMAAAAAAIeaVq4EAAAAzYEjlwAAAACpI7esBAAAAGCBApcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAi3oUBAAAAANDLHgAAAAAAU4fbQL5xIWf9blCh5sD10YfqZAvzAnPwplgcRJuxQvf3Zm039PDKro9dj6e3WMEQm9Agfsqkhmo7/XPs1aowdm1n24wGIIi6EBnjbMTjANsUf1jgSBNts4SDaolOgPOrBpuIV/6rgYT7aH9jRhjANdrEOdwa6ztVmKDwAAAAAAHG+nrzvtutOj1l82qryXQxsbvkwtL24OR8pgIDRS9dYRMAAABqY3YwLjIuMCx3aGFsZS1zdWl0AAAAAA==";
    const INVOKE_LOG: &str = "Program darkr3FB87qAZmgLwKov6Hk9Yiah5UT4rUYu8Zhthw1 invoke [1]";

    #[test]
    fn settle_event_emits_swap_with_realised_amounts() {
        let mut state = State::new();
        assert!(state.handle_log(INVOKE_LOG).is_none(), "invoke is a header, not a swap");
        let swap = state
            .handle_log(SETTLE_LOG)
            .expect("Settle event should decode into a swap");

        assert_eq!(swap.protocol, pb::Protocol::Darklake as i32);
        assert_eq!(swap.input_amount, 10_000_000);
        assert_eq!(swap.output_amount, 1_256_182);
        // direction=1 -> input = token_mint_x = wSOL (So11111111111111111111111111111111111111111)
        assert_eq!(swap.input_mint, crate::SOL_MINT.to_vec());
        // trader db07cb09899937bf5f53a84eaa28da7932d446d9d6fce456dad70e20eb9c1e22
        assert_eq!(
            swap.user,
            [
                0xdb, 0x07, 0xcb, 0x09, 0x89, 0x99, 0x37, 0xbf, 0x5f, 0x53, 0xa8, 0x4e, 0xaa, 0x28, 0xda, 0x79,
                0x32, 0xd4, 0x46, 0xd9, 0xd6, 0xfc, 0xe4, 0x56, 0xda, 0xd7, 0x0e, 0x20, 0xeb, 0x9c, 0x1e, 0x22,
            ]
        );
    }

    #[test]
    fn swap_intent_event_is_ignored() {
        // Real SwapEvent (intent) from mainnet tx
        // 4KpMwB5n8u3jmzcTTUD7NJMbNC2SJA9hURa2THE9mFqSJUGqC6UYinCGpkK9qZ8FdxH6V4fkqHuJWAfkmhTtQFJn —
        // only realised SettleEvents should produce swaps.
        let swap_event_log = "Program data: UWzjvs3QCsQLE/3EW00Z3UD/couPlpXnLjrnbc03ZEIcKMvzILA+OAFLE74XAAAAALqDAAAAAAAA3UEAAAAAAAAjZWYAAAAAAF0uDQAAAAAA3ehmAAAAAADI60EAAAAAAF0uDQAAAAAAlOqXAAAAAAAbZSEAAAAAANMIWgAAAAAALnITAAAAAAAzBz0AAAAAAF0uDQAAAAAA3ehmAAAAAAAAAAAAAAAAAI7aAAAAAAAAkMQAAAAAAADjBXni+/SMviQjZ16c3j/3bO0rtEbrozGeXFo12dr3SU55lnjhWzDrD1x9AiUrbwXOO8hFUaKgZYCIx+JuuPuHNJAPPbJjColiTw3TN8i+TYy3K/EcIbh3z6DzXqHNu9sGm4hX/quBhPtof2NGGMA12sQ53BrrO1WYoPAAAAAAAc4BDmCv7bInF71jGS9UFFo/llozu4LSxwKess4eIIJkAAAAAA==";
        let mut state = State::new();
        assert!(state.handle_log(INVOKE_LOG).is_none());
        assert!(state.handle_log(swap_event_log).is_none(),
                "SwapEvent (intent) must not be matched");
    }
}
