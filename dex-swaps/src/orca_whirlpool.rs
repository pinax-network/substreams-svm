use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::orca;

use crate::logs::{scoped_program_log, ProgramLog};
use crate::token_mints::TokenMintLookup;

pub(crate) struct State {
    pending: Vec<InstructionSwap>,
    next_index: usize,
    is_invoked: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            pending: Vec::new(),
            next_index: 0,
            is_invoked: false,
        }
    }

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView, token_mints: &TokenMintLookup) {
        if let Some(swap) = decode_instruction(ix, Some(token_mints)) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &orca::whirlpool::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let event = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        // Legacy `Swap` only exposes `token_vault_a` / `token_vault_b` in its
        // accounts list; mints had to be resolved via `TokenMintLookup` in
        // `handle_instruction`. If that lookup missed, skip emitting this
        // row (we still consumed `next_index` so subsequent swaps stay
        // aligned with their logs).
        let mint_a = instruction.mint_a.clone()?;
        let mint_b = instruction.mint_b.clone()?;

        let (input_mint, output_mint) = if instruction.a_to_b { (mint_a, mint_b) } else { (mint_b, mint_a) };

        Some(pb::Swap {
            protocol: pb::Protocol::OrcaWhirlpool as i32,
            program_id: orca::whirlpool::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: orca::whirlpool::PROGRAM_ID.to_vec(),
            amm_pool: instruction.whirlpool.clone(),
            user: instruction.user.clone(),
            input_mint,
            input_amount: event.input_amount,
            output_mint,
            output_amount: event.output_amount,
        })
    }
}

#[cfg_attr(test, derive(Clone))]
struct InstructionSwap {
    stack_height: u32,
    user: Vec<u8>,
    whirlpool: Vec<u8>,
    /// Mint resolved from `token_vault_a` (legacy) or read from
    /// `token_mint_a` directly (V2). `None` only when the legacy `Swap`
    /// variant could not resolve the vault via `TokenMintLookup`.
    mint_a: Option<Vec<u8>>,
    /// See `mint_a`.
    mint_b: Option<Vec<u8>>,
    a_to_b: bool,
}

struct LogSwap {
    input_amount: u64,
    output_amount: u64,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    // `routed_pool::Tracker::observe` only needs the pool address; mints are
    // resolved later in `handle_instruction` when `TokenMintLookup` is in scope.
    decode_instruction(ix, None).map(|s| s.whirlpool)
}

fn decode_instruction(ix: &InstructionView, token_mints: Option<&TokenMintLookup>) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &orca::whirlpool::PROGRAM_ID {
        return None;
    }

    match orca::whirlpool::instructions::unpack(ix.data()) {
        Ok(orca::whirlpool::instructions::WhirlpoolInstruction::Swap(event)) => {
            // Legacy `swap` only carries vaults — `token_vault_a` and
            // `token_vault_b` — in its account list. Pre-fix this adapter
            // wrote those vault addresses straight into the `mint_a`/`mint_b`
            // slots, leaking pool token accounts into the protobuf's mint
            // fields. Resolve via `TokenMintLookup`; emit `None` placeholders
            // when the lookup misses so `handle_log` skips the row but stays
            // sequentially aligned.
            let accounts = orca::whirlpool::accounts::get_swap_accounts(ix).ok()?;
            let (mint_a, mint_b) = match token_mints {
                Some(lookup) => (
                    lookup.mint_for(accounts.token_vault_a.to_bytes().as_ref()),
                    lookup.mint_for(accounts.token_vault_b.to_bytes().as_ref()),
                ),
                None => (None, None),
            };
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.token_authority.to_bytes().to_vec(),
                whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                mint_a,
                mint_b,
                a_to_b: event.a_to_b,
            })
        }
        Ok(orca::whirlpool::instructions::WhirlpoolInstruction::SwapV2(event)) => {
            // SwapV2 exposes mint accounts directly — no vault→mint resolution
            // step needed.
            let accounts = orca::whirlpool::accounts::get_swap_v2_accounts(ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.token_authority.to_bytes().to_vec(),
                whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                mint_a: Some(accounts.token_mint_a.to_bytes().to_vec()),
                mint_b: Some(accounts.token_mint_b.to_bytes().to_vec()),
                a_to_b: event.a_to_b,
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_program_data(log_message)?;
    match orca::whirlpool::events::parse_event(data.as_slice()) {
        Ok(orca::whirlpool::events::WhirlpoolEvent::Traded(event)) => Some(LogSwap {
            input_amount: event.input_amount,
            output_amount: event.output_amount,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::base58;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, TokenBalance, Transaction,
        TransactionStatusMeta, UiTokenAmount,
    };

    /// Whirlpool `swap` discriminator from the IDL.
    const SWAP_DISC: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];

    /// SwapInstruction args body — values irrelevant for these tests.
    fn swap_body(a_to_b: bool) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes()); // amount
        b.extend_from_slice(&0u64.to_le_bytes()); // other_amount_threshold
        b.extend_from_slice(&0u128.to_le_bytes()); // sqrt_price_limit
        b.push(0u8); // amount_specified_is_input
        b.push(if a_to_b { 1 } else { 0 });
        b
    }

    fn make_tx(accounts: &[[u8; 32]], token_balances: &[(u32, [u8; 32])]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = orca::whirlpool::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8);
        }
        let mut data = SWAP_DISC.to_vec();
        data.extend_from_slice(&swap_body(true));

        let pre_token_balances = token_balances
            .iter()
            .map(|(idx, mint)| TokenBalance {
                account_index: *idx,
                mint: base58::encode(mint),
                owner: "".to_string(),
                program_id: "".to_string(),
                ui_token_amount: Some(UiTokenAmount::default()),
            })
            .collect();

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
            meta: Some(TransactionStatusMeta {
                pre_token_balances,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn legacy_swap_resolves_vaults_to_mints() {
        // SwapAccounts layout: token_program, token_authority, whirlpool,
        // token_owner_account_a, token_vault_a, token_owner_account_b,
        // token_vault_b, tick_array0, tick_array1, tick_array2, oracle.
        let token_program = [0x00; 32];
        let token_authority = [0x01; 32];
        let whirlpool = [0x02; 32];
        let owner_a = [0x03; 32];
        let vault_a = [0x04; 32];
        let owner_b = [0x05; 32];
        let vault_b = [0x06; 32];
        let tick0 = [0x07; 32];
        let tick1 = [0x08; 32];
        let tick2 = [0x09; 32];
        let oracle = [0x0a; 32];

        let mint_a = [0xaa; 32];
        let mint_b = [0xbb; 32];
        // vault_a sits at account_keys[4 + 2] = 6, vault_b at [6 + 2] = 8.
        let token_balances: &[(u32, [u8; 32])] = &[(6, mint_a), (8, mint_b)];

        let tx = make_tx(
            &[
                token_program,
                token_authority,
                whirlpool,
                owner_a,
                vault_a,
                owner_b,
                vault_b,
                tick0,
                tick1,
                tick2,
                oracle,
            ],
            token_balances,
        );
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("legacy swap must decode");
        assert_eq!(swap.mint_a.as_deref(), Some(mint_a.as_slice()), "mint_a must be resolved mint, not vault");
        assert_eq!(swap.mint_b.as_deref(), Some(mint_b.as_slice()), "mint_b must be resolved mint, not vault");
        assert_eq!(swap.whirlpool, whirlpool.to_vec());

        // Regression: vault address must NOT leak into the mint slot.
        assert_ne!(swap.mint_a.as_deref(), Some(vault_a.as_slice()));
        assert_ne!(swap.mint_b.as_deref(), Some(vault_b.as_slice()));
    }

    #[test]
    fn legacy_swap_unresolved_lookup_keeps_placeholder_with_none() {
        let accounts: Vec<[u8; 32]> = (0u8..11u8).map(|i| [i + 1; 32]).collect();
        let tx = make_tx(&accounts, &[]);
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("placeholder must still emit");
        assert!(swap.mint_a.is_none(), "vault must not leak when lookup misses");
        assert!(swap.mint_b.is_none(), "vault must not leak when lookup misses");
    }
}
