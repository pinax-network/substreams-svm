use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpswap;

pub(crate) struct PendingTrade {
    pool: Vec<u8>,
    base_mint: Vec<u8>,
    quote_mint: Vec<u8>,
}

struct TradeEvent {
    is_buy: bool,
    user: Vec<u8>,
    base_amount: u64,
    quote_amount: u64,
}

// PumpSwap event discriminators (Anchor anchor_disc).
// Stable since the original Pump.fun AMM program — the program has only ever
// extended the payload, never changed these tags.
const BUY_EVENT: [u8; 8] = [103, 244, 82, 31, 44, 245, 119, 119];
const SELL_EVENT: [u8; 8] = [62, 47, 55, 10, 165, 3, 220, 42];

// Anchor self-CPI invocation tag — prepended to events emitted via `emit_cpi!`.
const ANCHOR_SELF_CPI_TAG: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

// Stable byte offsets within the BuyEvent / SellEvent payload (after the
// anchor_disc has been stripped). The pump.fun AMM program has appended
// fields to these events at least three times since launch
// (BuyEventV1 → BuyEventV2 → PumpSwap with track_volume / cashback / ix_name),
// but the leading layout — `i64 timestamp` + 13×`u64` + 7×`pubkey` — has
// never been reordered or shrunk. We read the fields we need at fixed
// offsets and ignore everything that follows, which makes the decoder
// resilient to future program upgrades that append more fields.
const BASE_AMOUNT_OFFSET: usize = 8;        // 2nd field — `base_amount_(in|out)`
const QUOTE_AMOUNT_OFFSET: usize = 56;      // 8th field — `quote_amount_(in|out)`
const USER_OFFSET: usize = 144;             // 16th field — 2nd pubkey
const MIN_PAYLOAD_LEN: usize = USER_OFFSET + 32; // 176 bytes

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpswap::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    let event = decode_trade_event(instruction)?;
    let trade = pending_trade.take()?;
    let (input_amount, output_amount) = if event.is_buy {
        (event.quote_amount, event.base_amount)
    } else {
        (event.base_amount, event.quote_amount)
    };

    Some(pb::Swap {
        protocol: pb::Protocol::PumpfunAmm as i32,
        program_id: pumpswap::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: pumpswap::PROGRAM_ID.to_vec(),
        amm_pool: trade.pool,
        user: event.user,
        input_mint: if event.is_buy { trade.quote_mint.clone() } else { trade.base_mint.clone() },
        input_amount,
        output_mint: if event.is_buy { trade.base_mint } else { trade.quote_mint },
        output_amount,
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpswap::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(instruction).map(|t| t.pool)
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    // Instruction discriminators (BUY / SELL / BUY_EXACT_QUOTE_IN) and the
    // first 17 accounts are unchanged across the legacy `pumpfun::amm` IDL
    // and the newer `pumpswap` IDL.
    match pumpswap::instructions::unpack(instruction.data()) {
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_)) => Some(PendingTrade {
            pool: instruction.accounts().get(0)?.0.to_vec(),
            base_mint: instruction.accounts().get(4)?.0.to_vec(),
            quote_mint: instruction.accounts().get(5)?.0.to_vec(),
        }),
        _ => None,
    }
}

fn decode_trade_event(instruction: &InstructionView) -> Option<TradeEvent> {
    decode_event_payload(instruction.data())
}

fn decode_event_payload(data: &[u8]) -> Option<TradeEvent> {
    // Events are emitted via `emit_cpi!`, so the instruction data is laid out
    // as `[ANCHOR_SELF_CPI_TAG (8B)][event_disc (8B)][borsh-serialized event]`.
    // Strip both prefixes and decode the payload at fixed offsets.
    if data.len() < 16 {
        return None;
    }
    if data[..8] != ANCHOR_SELF_CPI_TAG {
        return None;
    }
    let event_disc: [u8; 8] = data[8..16].try_into().ok()?;
    let payload = &data[16..];

    let is_buy = match event_disc {
        BUY_EVENT => true,
        SELL_EVENT => false,
        _ => return None,
    };

    if payload.len() < MIN_PAYLOAD_LEN {
        return None;
    }

    let base_amount = u64::from_le_bytes(payload[BASE_AMOUNT_OFFSET..BASE_AMOUNT_OFFSET + 8].try_into().ok()?);
    let quote_amount = u64::from_le_bytes(payload[QUOTE_AMOUNT_OFFSET..QUOTE_AMOUNT_OFFSET + 8].try_into().ok()?);
    let user = payload[USER_OFFSET..USER_OFFSET + 32].to_vec();

    Some(TradeEvent {
        is_buy,
        user,
        base_amount,
        quote_amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic CPI event payload: anchor tag + event disc + leading layout.
    /// Trailing bytes simulate program upgrades appending fields.
    fn make_event(disc: [u8; 8], base: u64, quote: u64, user: [u8; 32], trailing: &[u8]) -> Vec<u8> {
        let mut data = Vec::with_capacity(16 + 176 + trailing.len());
        data.extend_from_slice(&ANCHOR_SELF_CPI_TAG);
        data.extend_from_slice(&disc);
        // payload (176 bytes for the stable leading layout)
        data.extend_from_slice(&0i64.to_le_bytes());                   // timestamp
        data.extend_from_slice(&base.to_le_bytes());                   // 2nd field
        for _ in 0..5 { data.extend_from_slice(&0u64.to_le_bytes()); } // 3rd-7th fields
        data.extend_from_slice(&quote.to_le_bytes());                  // 8th field
        for _ in 0..6 { data.extend_from_slice(&0u64.to_le_bytes()); } // 9th-14th fields
        data.extend_from_slice(&[0u8; 32]);                            // pool (1st pubkey)
        data.extend_from_slice(&user);                                 // user (2nd pubkey)
        data.extend_from_slice(trailing);
        data
    }

    fn user_pk(byte: u8) -> [u8; 32] { [byte; 32] }

    #[test]
    fn decodes_buy_event_minimal_payload() {
        let data = make_event(BUY_EVENT, 1_000, 999, user_pk(0xab), &[]);
        let ev = decode_event_payload(&data).unwrap();
        assert!(ev.is_buy);
        assert_eq!(ev.base_amount, 1_000);
        assert_eq!(ev.quote_amount, 999);
        assert_eq!(ev.user, user_pk(0xab).to_vec());
    }

    #[test]
    fn decodes_sell_event_minimal_payload() {
        let data = make_event(SELL_EVENT, 7, 13, user_pk(0xcd), &[]);
        let ev = decode_event_payload(&data).unwrap();
        assert!(!ev.is_buy);
        assert_eq!(ev.base_amount, 7);
        assert_eq!(ev.quote_amount, 13);
    }

    #[test]
    fn ignores_trailing_bytes_v2_layout() {
        // V2 appended coin_creator (32) + 2 u64 fees = 48 bytes after the leading 176.
        let trailing = vec![0u8; 48];
        let data = make_event(BUY_EVENT, 100, 50, user_pk(0x01), &trailing);
        let ev = decode_event_payload(&data).unwrap();
        assert_eq!(ev.base_amount, 100);
        assert_eq!(ev.quote_amount, 50);
    }

    #[test]
    fn ignores_trailing_bytes_pumpswap_layout() {
        // PumpSwap (post-2026-02-12) appended bool + 5 u64 + string + 2 u64 cashback.
        // The exact size doesn't matter — decoder ignores whatever follows the leading 176.
        let trailing = vec![0u8; 224];
        let data = make_event(BUY_EVENT, 42, 84, user_pk(0x02), &trailing);
        let ev = decode_event_payload(&data).unwrap();
        assert_eq!(ev.base_amount, 42);
        assert_eq!(ev.quote_amount, 84);
    }

    #[test]
    fn rejects_data_without_anchor_cpi_tag() {
        // Same shape, but the leading 8 bytes are not the anchor self-CPI tag.
        let mut data = make_event(BUY_EVENT, 1, 1, user_pk(0xff), &[]);
        data[..8].copy_from_slice(&[0u8; 8]);
        assert!(decode_event_payload(&data).is_none());
    }

    #[test]
    fn rejects_unknown_event_discriminator() {
        let unknown_disc = [0xaa; 8];
        let data = make_event(unknown_disc, 1, 1, user_pk(0xff), &[]);
        assert!(decode_event_payload(&data).is_none());
    }

    #[test]
    fn rejects_payload_shorter_than_leading_layout() {
        // Only 100 bytes of payload — short of the 176-byte minimum.
        let mut data = Vec::new();
        data.extend_from_slice(&ANCHOR_SELF_CPI_TAG);
        data.extend_from_slice(&BUY_EVENT);
        data.extend_from_slice(&vec![0u8; 100]);
        assert!(decode_event_payload(&data).is_none());
    }

    #[test]
    fn rejects_data_too_short_for_any_disc() {
        assert!(decode_event_payload(&[]).is_none());
        assert!(decode_event_payload(&ANCHOR_SELF_CPI_TAG).is_none()); // only the tag
    }

    #[test]
    fn buy_and_sell_use_same_offsets_with_inverted_meaning() {
        // For a Buy event: 2nd field = base_amount_OUT, 8th field = quote_amount_IN.
        // For a Sell event: 2nd field = base_amount_IN, 8th field = quote_amount_OUT.
        // The decoder simply reports them as `base_amount` / `quote_amount`; the caller
        // inverts based on `is_buy`. Verify both directions extract the same numerical
        // values from the same byte positions.
        let buy = decode_event_payload(&make_event(BUY_EVENT, 1_000, 9, user_pk(0x10), &[])).unwrap();
        let sell = decode_event_payload(&make_event(SELL_EVENT, 1_000, 9, user_pk(0x11), &[])).unwrap();
        assert_eq!(buy.base_amount, sell.base_amount);
        assert_eq!(buy.quote_amount, sell.quote_amount);
        assert!(buy.is_buy);
        assert!(!sell.is_buy);
    }
}
