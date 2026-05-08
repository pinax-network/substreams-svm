use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpswap;
use substreams_solana_idls::pumpswap::accounts::TradeAccounts;
use substreams_solana_idls::pumpswap::events::TradeDirection;

pub(crate) struct PendingTrade {
    pool: Vec<u8>,
    base_mint: Vec<u8>,
    quote_mint: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpswap::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in `substreams_solana_idls::pumpswap`
    // — read the comment there for the rationale (pump.fun has appended fields
    // to BuyEvent/SellEvent multiple times since launch and we don't want each
    // sink chasing the IDL).
    let event = pumpswap::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    let is_buy = matches!(event.direction, TradeDirection::Buy);
    let (input_amount, output_amount) = if is_buy {
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
        user: event.user.to_vec(),
        input_mint: if is_buy { trade.quote_mint.clone() } else { trade.base_mint.clone() },
        input_amount,
        output_mint: if is_buy { trade.base_mint } else { trade.quote_mint },
        output_amount,
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpswap::PROGRAM_ID {
        return None;
    }
    // `routed_pool::Tracker::observe` only needs the pool address (accounts[0])
    // for cross-protocol routing attribution. We deliberately don't run the
    // full strict `TradeAccounts::try_from` here — pre-filter inner CPI
    // fixtures may carry only the pool slot, and the strict path would reject
    // them with `Missing` for trailing accounts.
    if !is_trade_instruction(instruction) {
        return None;
    }
    instruction.accounts().get(0).map(|a| a.0.to_vec())
}

fn is_trade_instruction(instruction: &InstructionView) -> bool {
    matches!(
        pumpswap::instructions::unpack(instruction.data()),
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_))
    )
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    match pumpswap::instructions::unpack(instruction.data()) {
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_)) => {
            // IDL-canonical account layout — see the comment + IDX_* constants
            // in `substreams_solana_idls::pumpswap::accounts`. Hand-indexing
            // `accounts.get(N)` here previously caused an off-by-one
            // (`base_mint` = accounts[4], should have been [3]) which let the
            // user's *base token account* leak into the `mint` slot,
            // producing a long tail of fake `(mint0, mint1)` pairs per pool
            // downstream.
            let accounts = TradeAccounts::try_from(instruction).ok()?;
            Some(PendingTrade {
                pool: accounts.pool.to_bytes().to_vec(),
                base_mint: accounts.base_mint.to_bytes().to_vec(),
                quote_mint: accounts.quote_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, Transaction, TransactionStatusMeta,
    };

    /// PumpSwap `buy` instruction discriminator from `pumpswap/idl.json`.
    const BUY_DISC: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
    /// PumpSwap `sell` instruction discriminator.
    const SELL_DISC: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

    fn buy_body() -> Vec<u8> {
        // BuyInstruction { base_amount_out: u64, max_quote_amount_in: u64 }
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b
    }

    fn sell_body() -> Vec<u8> {
        // SellInstruction { base_amount_in: u64, min_quote_amount_out: u64 }
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b
    }

    /// Build a ConfirmedTransaction with one PumpSwap instruction.
    fn make_tx(disc: [u8; 8], body: Vec<u8>, accounts: &[[u8; 32]]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = pumpswap::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8);
        }
        let mut data = disc.to_vec();
        data.extend_from_slice(&body);

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

    #[test]
    fn buy_decode_resolves_canonical_mints() {
        let pool = [0x01; 32];
        let user = [0x02; 32];
        let global_config = [0x03; 32];
        let base_mint = [0x04; 32];
        let quote_mint = [0x05; 32];
        let user_base_ta = [0x06; 32];
        let user_quote_ta = [0x07; 32];
        let pool_base_ta = [0x08; 32];
        let pool_quote_ta = [0x09; 32];
        let protocol_fee_recipient = [0x0a; 32];
        let protocol_fee_recipient_ta = [0x0b; 32];

        let tx = make_tx(
            BUY_DISC,
            buy_body(),
            &[
                pool,
                user,
                global_config,
                base_mint,
                quote_mint,
                user_base_ta,
                user_quote_ta,
                pool_base_ta,
                pool_quote_ta,
                protocol_fee_recipient,
                protocol_fee_recipient_ta,
            ],
        );
        let ix = tx.walk_instructions().next().unwrap();
        let trade = decode_trade_instruction(&ix).expect("buy instruction must decode");

        assert_eq!(trade.pool, pool.to_vec());
        assert_eq!(trade.base_mint, base_mint.to_vec(), "base_mint must come from accounts[3], not [4]");
        assert_eq!(trade.quote_mint, quote_mint.to_vec(), "quote_mint must come from accounts[4], not [5]");

        // Regression: the previous adapter read accounts[4]/[5] which is
        // exactly (quote_mint, user_base_token_account). Make sure neither
        // slot leaks the user's TA back in.
        assert_ne!(trade.base_mint, user_base_ta.to_vec());
        assert_ne!(trade.quote_mint, user_base_ta.to_vec());
    }

    #[test]
    fn sell_decode_uses_same_layout_as_buy() {
        let pool = [0x21; 32];
        let user = [0x22; 32];
        let global_config = [0x23; 32];
        let base_mint = [0x24; 32];
        let quote_mint = [0x25; 32];
        let accounts: Vec<[u8; 32]> = vec![pool, user, global_config, base_mint, quote_mint]
            .into_iter()
            .chain((6u8..12u8).map(|i| [i; 32]))
            .collect();

        let tx = make_tx(SELL_DISC, sell_body(), &accounts);
        let ix = tx.walk_instructions().next().unwrap();
        let trade = decode_trade_instruction(&ix).expect("sell instruction must decode");

        assert_eq!(trade.pool, pool.to_vec());
        assert_eq!(trade.base_mint, base_mint.to_vec());
        assert_eq!(trade.quote_mint, quote_mint.to_vec());
    }
}
