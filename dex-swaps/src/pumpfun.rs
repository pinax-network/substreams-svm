use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpfun::bonding_curve as pumpfun;

use crate::SOL_MINT;

pub(crate) struct PendingTrade {
    bonding_curve: Vec<u8>,
}

struct TradeEvent {
    mint: Vec<u8>,
    sol_amount: u64,
    token_amount: u64,
    is_buy: bool,
    user: Vec<u8>,
}

// Pump.fun bonding-curve TradeEvent anchor discriminator. Stable since launch.
const TRADE_EVENT: [u8; 8] = [189, 219, 127, 211, 78, 230, 97, 238];
const ANCHOR_SELF_CPI_TAG: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

// Stable byte offsets within the TradeEvent payload (after the 16-byte
// preamble has been stripped). The pump.fun bonding curve TradeEvent has
// gone through V0..V3 (and now V4-shape with `ix_name` / cashback / extra
// volume fields, ~303-byte payload), but the leading layout — `mint` +
// `sol_amount` + `token_amount` + `is_buy` + `user` — has been stable
// since V0. Reading at fixed offsets and ignoring trailing bytes makes
// the decoder resilient to future append-only schema bumps.
const MINT_OFFSET: usize = 0;
const SOL_AMOUNT_OFFSET: usize = 32;
const TOKEN_AMOUNT_OFFSET: usize = 40;
const IS_BUY_OFFSET: usize = 48;
const USER_OFFSET: usize = 49;
const MIN_PAYLOAD_LEN: usize = USER_OFFSET + 32; // 81 bytes

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpfun::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    let event = decode_trade_event(instruction)?;
    let trade = pending_trade.take()?;

    Some(pb::Swap {
        protocol: pb::Protocol::Pumpfun as i32,
        program_id: pumpfun::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: pumpfun::PROGRAM_ID.to_vec(),
        amm_pool: trade.bonding_curve,
        user: event.user,
        input_mint: if event.is_buy { SOL_MINT.to_vec() } else { event.mint.clone() },
        input_amount: if event.is_buy { event.sol_amount } else { event.token_amount },
        output_mint: if event.is_buy { event.mint } else { SOL_MINT.to_vec() },
        output_amount: if event.is_buy { event.token_amount } else { event.sol_amount },
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpfun::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(instruction).map(|t| t.bonding_curve)
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    match pumpfun::instructions::unpack(instruction.data()) {
        Ok(pumpfun::instructions::PumpFunInstruction::Buy(_))
        | Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)) => Some(PendingTrade {
            bonding_curve: instruction.accounts().get(3)?.0.to_vec(),
        }),
        _ => None,
    }
}

fn decode_trade_event(instruction: &InstructionView) -> Option<TradeEvent> {
    decode_event_payload(instruction.data())
}

fn decode_event_payload(data: &[u8]) -> Option<TradeEvent> {
    // emit_cpi! invocation: [ANCHOR_SELF_CPI_TAG (8B)][TRADE_EVENT (8B)][payload]
    if data.len() < 16 {
        return None;
    }
    if data[..8] != ANCHOR_SELF_CPI_TAG {
        return None;
    }
    let event_disc: [u8; 8] = data[8..16].try_into().ok()?;
    if event_disc != TRADE_EVENT {
        return None;
    }
    let payload = &data[16..];
    if payload.len() < MIN_PAYLOAD_LEN {
        return None;
    }

    let mint = payload[MINT_OFFSET..MINT_OFFSET + 32].to_vec();
    let sol_amount = u64::from_le_bytes(payload[SOL_AMOUNT_OFFSET..SOL_AMOUNT_OFFSET + 8].try_into().ok()?);
    let token_amount = u64::from_le_bytes(payload[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8].try_into().ok()?);
    let is_buy = match payload[IS_BUY_OFFSET] {
        0 => false,
        1 => true,
        _ => return None, // bool is strictly 0 or 1 in Borsh
    };
    let user = payload[USER_OFFSET..USER_OFFSET + 32].to_vec();

    Some(TradeEvent {
        mint,
        sol_amount,
        token_amount,
        is_buy,
        user,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(is_buy: bool, mint: [u8; 32], user: [u8; 32], sol: u64, token: u64, trailing: &[u8]) -> Vec<u8> {
        let mut data = Vec::with_capacity(16 + 81 + trailing.len());
        data.extend_from_slice(&ANCHOR_SELF_CPI_TAG);
        data.extend_from_slice(&TRADE_EVENT);
        data.extend_from_slice(&mint);                                      // 32
        data.extend_from_slice(&sol.to_le_bytes());                         // 8
        data.extend_from_slice(&token.to_le_bytes());                       // 8
        data.push(if is_buy { 1 } else { 0 });                              // 1
        data.extend_from_slice(&user);                                      // 32
        data.extend_from_slice(trailing);
        data
    }

    fn pk(b: u8) -> [u8; 32] { [b; 32] }

    #[test]
    fn decodes_buy_with_v0_payload() {
        let data = make_event(true, pk(0xa1), pk(0xa2), 1_000_000_000, 50_000_000, &[]);
        let ev = decode_event_payload(&data).unwrap();
        assert!(ev.is_buy);
        assert_eq!(ev.sol_amount, 1_000_000_000);
        assert_eq!(ev.token_amount, 50_000_000);
        assert_eq!(ev.mint, pk(0xa1).to_vec());
        assert_eq!(ev.user, pk(0xa2).to_vec());
    }

    #[test]
    fn decodes_sell_with_v0_payload() {
        let data = make_event(false, pk(0xb1), pk(0xb2), 7, 13, &[]);
        let ev = decode_event_payload(&data).unwrap();
        assert!(!ev.is_buy);
        assert_eq!(ev.sol_amount, 7);
        assert_eq!(ev.token_amount, 13);
    }

    #[test]
    fn ignores_trailing_v3_fields() {
        // V3 appends timestamp + 4 reserves + fee_recipient + 2 fee fields + creator + 2 creator fees
        // + bool track_volume + 4 volume tokens + last_update_timestamp = lots of bytes.
        let trailing = vec![0u8; 169]; // V3 payload length minus V0 (250 - 81)
        let data = make_event(true, pk(0xc1), pk(0xc2), 100, 200, &trailing);
        let ev = decode_event_payload(&data).unwrap();
        assert_eq!(ev.sol_amount, 100);
        assert_eq!(ev.token_amount, 200);
    }

    #[test]
    fn ignores_trailing_post_v3_fields() {
        // Post-V3 appends ix_name (String) + cashback fields + more.
        let trailing = vec![0u8; 250];
        let data = make_event(true, pk(0xd1), pk(0xd2), 42, 84, &trailing);
        let ev = decode_event_payload(&data).unwrap();
        assert_eq!(ev.sol_amount, 42);
        assert_eq!(ev.token_amount, 84);
    }

    #[test]
    fn rejects_non_trade_event_disc() {
        // Replace TRADE_EVENT with arbitrary 8 bytes.
        let mut data = make_event(true, pk(0), pk(0), 1, 1, &[]);
        data[8..16].copy_from_slice(&[0xff; 8]);
        assert!(decode_event_payload(&data).is_none());
    }

    #[test]
    fn rejects_data_without_anchor_cpi_tag() {
        let mut data = make_event(true, pk(0), pk(0), 1, 1, &[]);
        data[..8].fill(0);
        assert!(decode_event_payload(&data).is_none());
    }

    #[test]
    fn rejects_short_payload() {
        let mut data = Vec::new();
        data.extend_from_slice(&ANCHOR_SELF_CPI_TAG);
        data.extend_from_slice(&TRADE_EVENT);
        data.extend_from_slice(&[0u8; 50]); // shorter than 81-byte minimum
        assert!(decode_event_payload(&data).is_none());
    }

    #[test]
    fn rejects_invalid_is_buy_byte() {
        let mut data = make_event(true, pk(0), pk(0), 1, 1, &[]);
        // is_buy lives at offset 16 (preamble) + 48 (within payload) = 64
        data[16 + IS_BUY_OFFSET] = 2;
        assert!(decode_event_payload(&data).is_none());
    }
}
