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
        // `Swap` / `SwapV2` push exactly one PendingSwap; `TwoHopSwap` /
        // `TwoHopSwapV2` push two (one per hop) which are consumed in-order
        // by the next two `Traded` events.
        for swap in decode_instructions(ix, Some(token_mints)) {
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

        // Legacy `Swap` / `TwoHopSwap` can have unresolved mints when the
        // tx's pre/post token balances don't reference the relevant vault.
        // Skip emitting (next_index already advanced for sequential
        // alignment with subsequent swaps in the same tx).
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
    /// Mint resolved from `token_vault_a` (legacy variants) or read from
    /// `token_mint_a` directly (V2 variants). `None` only when the legacy
    /// vault lookup missed.
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
    // For routed-pool tracking we only need any pool address touched. For
    // `TwoHopSwap*` we return the first hop's whirlpool — Jupiter v6 doesn't
    // route through Orca's two-hop instruction (it does its own multi-hop),
    // so this is a non-issue in practice.
    decode_instructions(ix, None).into_iter().next().map(|s| s.whirlpool)
}

fn decode_instructions(ix: &InstructionView, token_mints: Option<&TokenMintLookup>) -> Vec<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &orca::whirlpool::PROGRAM_ID {
        return Vec::new();
    }

    let parsed = match orca::whirlpool::instructions::unpack(ix.data()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    match parsed {
        orca::whirlpool::instructions::WhirlpoolInstruction::Swap(event) => {
            // Legacy `swap` only carries vaults — `token_vault_a` and
            // `token_vault_b` — in its account list. Pre-v0.5.0 this adapter
            // wrote vault addresses straight into the `mint_a`/`mint_b`
            // slots; now we resolve via `TokenMintLookup`.
            let accounts = match orca::whirlpool::accounts::get_swap_accounts(ix) {
                Ok(a) => a,
                Err(_) => return Vec::new(),
            };
            let (mint_a, mint_b) = resolve_vaults(token_mints, accounts.token_vault_a.as_ref(), accounts.token_vault_b.as_ref());
            vec![InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.token_authority.to_bytes().to_vec(),
                whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                mint_a,
                mint_b,
                a_to_b: event.a_to_b,
            }]
        }
        orca::whirlpool::instructions::WhirlpoolInstruction::SwapV2(event) => {
            let accounts = match orca::whirlpool::accounts::get_swap_v2_accounts(ix) {
                Ok(a) => a,
                Err(_) => return Vec::new(),
            };
            vec![InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.token_authority.to_bytes().to_vec(),
                whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                mint_a: Some(accounts.token_mint_a.to_bytes().to_vec()),
                mint_b: Some(accounts.token_mint_b.to_bytes().to_vec()),
                a_to_b: event.a_to_b,
            }]
        }
        orca::whirlpool::instructions::WhirlpoolInstruction::TwoHopSwap(event) => {
            // Single instruction that emits two `Traded` events back-to-back.
            // Push one PendingSwap per hop so the sequential `next_index`
            // matcher in `handle_log` consumes them in order.
            // Legacy form carries vaults only — resolve via TokenMintLookup.
            let accounts = match orca::whirlpool::accounts::get_two_hop_swap_accounts(ix) {
                Ok(a) => a,
                Err(_) => return Vec::new(),
            };
            let (hop1_a, hop1_b) = resolve_vaults(token_mints, accounts.token_vault_one_a.as_ref(), accounts.token_vault_one_b.as_ref());
            let (hop2_a, hop2_b) = resolve_vaults(token_mints, accounts.token_vault_two_a.as_ref(), accounts.token_vault_two_b.as_ref());
            vec![
                InstructionSwap {
                    stack_height: ix.stack_height(),
                    user: accounts.token_authority.to_bytes().to_vec(),
                    whirlpool: accounts.whirlpool_one.to_bytes().to_vec(),
                    mint_a: hop1_a,
                    mint_b: hop1_b,
                    a_to_b: event.a_to_b_one,
                },
                InstructionSwap {
                    stack_height: ix.stack_height(),
                    user: accounts.token_authority.to_bytes().to_vec(),
                    whirlpool: accounts.whirlpool_two.to_bytes().to_vec(),
                    mint_a: hop2_a,
                    mint_b: hop2_b,
                    a_to_b: event.a_to_b_two,
                },
            ]
        }
        orca::whirlpool::instructions::WhirlpoolInstruction::TwoHopSwapV2(event) => {
            // V2 carries explicit `token_mint_input` / `token_mint_intermediate`
            // / `token_mint_output`. Per Orca convention `a_to_b_one`
            // determines hop 1's direction over `whirlpool_one`'s
            // (mint_a, mint_b). When `a_to_b_one=true`, input flows from
            // pool's a → b, so mint_a is the user's input mint.
            let accounts = match orca::whirlpool::accounts::get_two_hop_swap_v2_accounts(ix) {
                Ok(a) => a,
                Err(_) => return Vec::new(),
            };
            let mint_input = accounts.token_mint_input.to_bytes().to_vec();
            let mint_intermediate = accounts.token_mint_intermediate.to_bytes().to_vec();
            let mint_output = accounts.token_mint_output.to_bytes().to_vec();
            let (hop1_a, hop1_b) = if event.a_to_b_one {
                (mint_input.clone(), mint_intermediate.clone())
            } else {
                (mint_intermediate.clone(), mint_input.clone())
            };
            let (hop2_a, hop2_b) = if event.a_to_b_two {
                (mint_intermediate, mint_output)
            } else {
                (mint_output, mint_intermediate)
            };
            vec![
                InstructionSwap {
                    stack_height: ix.stack_height(),
                    user: accounts.token_authority.to_bytes().to_vec(),
                    whirlpool: accounts.whirlpool_one.to_bytes().to_vec(),
                    mint_a: Some(hop1_a),
                    mint_b: Some(hop1_b),
                    a_to_b: event.a_to_b_one,
                },
                InstructionSwap {
                    stack_height: ix.stack_height(),
                    user: accounts.token_authority.to_bytes().to_vec(),
                    whirlpool: accounts.whirlpool_two.to_bytes().to_vec(),
                    mint_a: Some(hop2_a),
                    mint_b: Some(hop2_b),
                    a_to_b: event.a_to_b_two,
                },
            ]
        }
        _ => Vec::new(),
    }
}

fn resolve_vaults(token_mints: Option<&TokenMintLookup>, vault_a: &[u8], vault_b: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    match token_mints {
        Some(lookup) => (lookup.mint_for(vault_a), lookup.mint_for(vault_b)),
        None => (None, None),
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
    /// Whirlpool `two_hop_swap` discriminator.
    const TWO_HOP_SWAP_DISC: [u8; 8] = [195, 96, 237, 108, 68, 162, 219, 230];

    /// SwapInstruction args body.
    fn swap_body(a_to_b: bool) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes()); // amount
        b.extend_from_slice(&0u64.to_le_bytes()); // other_amount_threshold
        b.extend_from_slice(&0u128.to_le_bytes()); // sqrt_price_limit
        b.push(0u8); // amount_specified_is_input
        b.push(if a_to_b { 1 } else { 0 });
        b
    }

    /// TwoHopSwapInstruction args body.
    fn two_hop_swap_body(a_to_b_one: bool, a_to_b_two: bool) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes()); // amount
        b.extend_from_slice(&0u64.to_le_bytes()); // other_amount_threshold
        b.push(0u8); // amount_specified_is_input
        b.push(if a_to_b_one { 1 } else { 0 });
        b.push(if a_to_b_two { 1 } else { 0 });
        b.extend_from_slice(&0u128.to_le_bytes()); // sqrt_price_limit_one
        b.extend_from_slice(&0u128.to_le_bytes()); // sqrt_price_limit_two
        b
    }

    fn make_tx(disc: [u8; 8], body: Vec<u8>, accounts: &[[u8; 32]], token_balances: &[(u32, [u8; 32])]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = orca::whirlpool::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8);
        }
        let mut data = disc.to_vec();
        data.extend_from_slice(&body);

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
        let token_balances: &[(u32, [u8; 32])] = &[(6, mint_a), (8, mint_b)];

        let tx = make_tx(
            SWAP_DISC,
            swap_body(true),
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

        let swaps = decode_instructions(&ix, Some(&mints));
        assert_eq!(swaps.len(), 1, "single swap must yield one InstructionSwap");
        let swap = &swaps[0];
        assert_eq!(swap.mint_a.as_deref(), Some(mint_a.as_slice()));
        assert_eq!(swap.mint_b.as_deref(), Some(mint_b.as_slice()));
        assert_eq!(swap.whirlpool, whirlpool.to_vec());
        assert_ne!(swap.mint_a.as_deref(), Some(vault_a.as_slice()));
        assert_ne!(swap.mint_b.as_deref(), Some(vault_b.as_slice()));
    }

    #[test]
    fn legacy_swap_unresolved_lookup_keeps_placeholder_with_none() {
        let accounts: Vec<[u8; 32]> = (0u8..11u8).map(|i| [i + 1; 32]).collect();
        let tx = make_tx(SWAP_DISC, swap_body(true), &accounts, &[]);
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swaps = decode_instructions(&ix, Some(&mints));
        assert_eq!(swaps.len(), 1);
        assert!(swaps[0].mint_a.is_none(), "vault must not leak when lookup misses");
        assert!(swaps[0].mint_b.is_none(), "vault must not leak when lookup misses");
    }

    #[test]
    fn two_hop_swap_legacy_emits_two_hops_with_resolved_mints() {
        // TwoHopSwap account layout: token_program, token_authority,
        // whirlpool_one, whirlpool_two, token_owner_account_one_a,
        // token_vault_one_a, token_owner_account_one_b, token_vault_one_b,
        // token_owner_account_two_a, token_vault_two_a,
        // token_owner_account_two_b, token_vault_two_b, tick_array_one_*,
        // tick_array_two_*, oracle_one, oracle_two.
        let token_program = [0x00; 32];
        let token_authority = [0x01; 32];
        let whirlpool_one = [0x02; 32];
        let whirlpool_two = [0x03; 32];
        let owner_one_a = [0x04; 32];
        let vault_one_a = [0x05; 32];
        let owner_one_b = [0x06; 32];
        let vault_one_b = [0x07; 32];
        let owner_two_a = [0x08; 32];
        let vault_two_a = [0x09; 32];
        let owner_two_b = [0x0a; 32];
        let vault_two_b = [0x0b; 32];
        let tick_array_one0 = [0x0c; 32];
        let tick_array_one1 = [0x0d; 32];
        let tick_array_one2 = [0x0e; 32];
        let tick_array_two0 = [0x0f; 32];
        let tick_array_two1 = [0x10; 32];
        let tick_array_two2 = [0x11; 32];
        let oracle_one = [0x12; 32];
        let oracle_two = [0x13; 32];

        let mint_one_a = [0xaa; 32];
        let mint_one_b = [0xbb; 32];
        let mint_two_a = [0xcc; 32];
        let mint_two_b = [0xdd; 32];
        // make_tx prepends fee_payer (0) and program (1) — instruction account
        // index N lives at account_keys[N + 2].
        let token_balances: &[(u32, [u8; 32])] = &[
            (5 + 2, mint_one_a),  // vault_one_a at instr-idx 5 -> key 7
            (7 + 2, mint_one_b),  // vault_one_b at instr-idx 7 -> key 9
            (9 + 2, mint_two_a),  // vault_two_a at instr-idx 9 -> key 11
            (11 + 2, mint_two_b), // vault_two_b at instr-idx 11 -> key 13
        ];

        let tx = make_tx(
            TWO_HOP_SWAP_DISC,
            two_hop_swap_body(true, false),
            &[
                token_program,
                token_authority,
                whirlpool_one,
                whirlpool_two,
                owner_one_a,
                vault_one_a,
                owner_one_b,
                vault_one_b,
                owner_two_a,
                vault_two_a,
                owner_two_b,
                vault_two_b,
                tick_array_one0,
                tick_array_one1,
                tick_array_one2,
                tick_array_two0,
                tick_array_two1,
                tick_array_two2,
                oracle_one,
                oracle_two,
            ],
            token_balances,
        );
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swaps = decode_instructions(&ix, Some(&mints));
        assert_eq!(swaps.len(), 2, "two_hop_swap must yield two InstructionSwaps");

        assert_eq!(swaps[0].whirlpool, whirlpool_one.to_vec());
        assert_eq!(swaps[0].mint_a.as_deref(), Some(mint_one_a.as_slice()));
        assert_eq!(swaps[0].mint_b.as_deref(), Some(mint_one_b.as_slice()));
        assert!(swaps[0].a_to_b);

        assert_eq!(swaps[1].whirlpool, whirlpool_two.to_vec());
        assert_eq!(swaps[1].mint_a.as_deref(), Some(mint_two_a.as_slice()));
        assert_eq!(swaps[1].mint_b.as_deref(), Some(mint_two_b.as_slice()));
        assert!(!swaps[1].a_to_b);
    }
}
