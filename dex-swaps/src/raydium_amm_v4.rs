use common::solana::parse_raydium_log;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::raydium;

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
            scoped_program_log(log_message, &raydium::amm::v4::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let log = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        // The swap instructions only carry the pool's *vaults* in their
        // account list — not the mints. We resolved them in
        // `handle_instruction` via `TokenMintLookup`. If the lookup missed
        // (no matching pre/post token balance for that vault), skip emitting
        // this row but keep the sequential alignment between pending
        // instructions and logs intact for any subsequent V4 swaps.
        let coin_mint = instruction.coin_mint.clone()?;
        let pc_mint = instruction.pc_mint.clone()?;

        // Per canonical Raydium AMM v4 source (`math.rs::SwapDirection`):
        //   PC2Coin = 1  (user gave pc, got coin → input=pc, output=coin)
        //   Coin2PC = 2  (user gave coin, got pc → input=coin, output=pc)
        // Pre-fix the conditional was inverted (`is_pc_to_coin = direction == 2`).
        // The bug was masked because vaults — not real mints — were stored,
        // so the swapped labels were equally "wrong" either way. Fixing the
        // vault→mint resolution exposes the inversion, so we correct both
        // here.
        let (input_mint, output_mint) = if log.direction == 2 {
            // Coin2PC
            (coin_mint, pc_mint)
        } else {
            // PC2Coin (and any other value defaults to the same orientation)
            (pc_mint, coin_mint)
        };

        Some(pb::Swap {
            protocol: pb::Protocol::RaydiumAmmV4 as i32,
            program_id: raydium::amm::v4::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::amm::v4::PROGRAM_ID.to_vec(),
            amm_pool: instruction.amm.clone(),
            user: instruction.user_source_owner.clone(),
            input_mint,
            input_amount: log.amount_in,
            output_mint,
            output_amount: log.amount_out,
        })
    }
}

#[cfg_attr(test, derive(Clone))]
struct InstructionSwap {
    stack_height: u32,
    amm: Vec<u8>,
    user_source_owner: Vec<u8>,
    /// Coin-side mint resolved from `pool_coin_token_account` via
    /// `TokenMintLookup`. `None` when the lookup missed (e.g. the tx's
    /// pre/post token balances didn't reference this vault). The placeholder
    /// is kept so `handle_log` can still match logs to instructions by
    /// sequential index — the row is just dropped at emit time.
    coin_mint: Option<Vec<u8>>,
    /// Pc-side mint resolved from `pool_pc_token_account`. See `coin_mint`.
    pc_mint: Option<Vec<u8>>,
}

struct LogSwap {
    direction: u64,
    amount_in: u64,
    amount_out: u64,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    // `routed_pool::Tracker::observe` only needs the pool address; mints are
    // resolved later in `handle_instruction` when `TokenMintLookup` is in scope.
    decode_instruction(ix, None).map(|s| s.amm)
}

fn decode_instruction(ix: &InstructionView, token_mints: Option<&TokenMintLookup>) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::amm::v4::PROGRAM_ID {
        return None;
    }

    match raydium::amm::v4::instructions::unpack(ix.data()) {
        Ok(raydium::amm::v4::instructions::RaydiumV4Instruction::SwapBaseIn(_))
        | Ok(raydium::amm::v4::instructions::RaydiumV4Instruction::SwapBaseOut(_)) => {
            // Use the IDL-canonical typed account helper. It transparently
            // handles both the post-fork (18-account, with `amm_target_orders`)
            // and legacy pre-fork (17-account) layouts.
            let accounts = raydium::amm::v4::accounts::get_swap_base_in_accounts(ix).ok()?;
            let (coin_mint, pc_mint) = resolve_vault_mints(token_mints, accounts.pool_coin_token_account.as_ref(), accounts.pool_pc_token_account.as_ref());
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                amm: accounts.amm.to_bytes().to_vec(),
                user_source_owner: accounts.user_source_owner.to_bytes().to_vec(),
                coin_mint,
                pc_mint,
            })
        }
        _ => None,
    }
}

fn resolve_vault_mints(token_mints: Option<&TokenMintLookup>, coin_vault: &[u8], pc_vault: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    match token_mints {
        Some(lookup) => (lookup.mint_for(coin_vault), lookup.mint_for(pc_vault)),
        None => (None, None),
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_raydium_log(log_message)?;
    match raydium::amm::v4::logs::unpack(data.as_slice()) {
        Ok(raydium::amm::v4::logs::RaydiumV4Log::SwapBaseIn(event)) => Some(LogSwap {
            direction: event.direction,
            amount_in: event.amount_in,
            amount_out: event.out_amount,
        }),
        Ok(raydium::amm::v4::logs::RaydiumV4Log::SwapBaseOut(event)) => Some(LogSwap {
            direction: event.direction,
            amount_in: event.deduct_in,
            amount_out: event.amount_out,
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

    /// SwapBaseIn discriminator (single byte for V4).
    const SWAP_BASE_IN_DISC: u8 = 9;

    /// Borsh-encoded SwapBaseIn payload (amounts irrelevant for these tests).
    fn swap_body() -> Vec<u8> {
        let mut b = vec![SWAP_BASE_IN_DISC];
        b.extend_from_slice(&0u64.to_le_bytes()); // amount_in
        b.extend_from_slice(&0u64.to_le_bytes()); // minimum_amount_out
        b
    }

    /// Build a ConfirmedTransaction with one Raydium AMM v4 instruction.
    /// `accounts` must be the per-instruction accounts in IDL order.
    /// `token_balances` populates pre_token_balances so `TokenMintLookup`
    /// can resolve vault → mint.
    fn make_tx(accounts: &[[u8; 32]], token_balances: &[(u32, [u8; 32])]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = raydium::amm::v4::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8);
        }
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
                        data: swap_body(),
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

    /// 18 accounts in post-fork IDL order.
    fn make_post_fork_accounts() -> ([[u8; 32]; 18], usize, usize) {
        // (accounts, pool_coin_account_keys_index, pool_pc_account_keys_index)
        let accs: [[u8; 32]; 18] = [
            [0x00; 32], // token_program
            [0x01; 32], // amm
            [0x02; 32], // amm_authority
            [0x03; 32], // amm_open_orders
            [0x04; 32], // amm_target_orders (post-fork only)
            [0x05; 32], // pool_coin_token_account ← VAULT
            [0x06; 32], // pool_pc_token_account ← VAULT
            [0x07; 32], // serum_program
            [0x08; 32], // serum_market
            [0x09; 32], // serum_bids
            [0x0a; 32], // serum_asks
            [0x0b; 32], // serum_event_queue
            [0x0c; 32], // serum_coin_vault_account
            [0x0d; 32], // serum_pc_vault_account
            [0x0e; 32], // serum_vault_signer
            [0x0f; 32], // uer_source_token_account
            [0x10; 32], // uer_destination_token_account
            [0x11; 32], // user_source_owner
        ];
        // make_tx prepends fee_payer (0) and program (1) — instruction account
        // index N lives at account_keys[N + 2].
        (accs, 5 + 2, 6 + 2)
    }

    #[test]
    fn post_fork_swap_resolves_vaults_to_mints() {
        let (accounts, coin_key_idx, pc_key_idx) = make_post_fork_accounts();
        // The actual mints we expect — distinct from the vaults to prove
        // resolution actually happened.
        let coin_mint = [0xaa; 32];
        let pc_mint = [0xbb; 32];

        let tx = make_tx(&accounts, &[(coin_key_idx as u32, coin_mint), (pc_key_idx as u32, pc_mint)]);
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("decoder must yield InstructionSwap");
        assert_eq!(swap.coin_mint.as_deref(), Some(coin_mint.as_slice()));
        assert_eq!(swap.pc_mint.as_deref(), Some(pc_mint.as_slice()));
        assert_eq!(swap.amm, accounts[1].to_vec());

        // Regression: the vault accounts themselves must not appear in the mint
        // slots. Pre-fix, the adapter wrote `accounts[5]` / `accounts[6]`
        // directly as `input_mint` / `output_mint`.
        assert_ne!(swap.coin_mint.as_deref(), Some(accounts[5].as_slice()));
        assert_ne!(swap.pc_mint.as_deref(), Some(accounts[6].as_slice()));
    }

    #[test]
    fn legacy_pre_fork_swap_resolves_vaults_to_mints() {
        // Pre-fork pools have 17 accounts (no amm_target_orders). Pool vaults
        // shift down: pool_coin_token_account at index 4, pool_pc at index 5.
        let accounts: [[u8; 32]; 17] = [
            [0x00; 32], // token_program
            [0x01; 32], // amm
            [0x02; 32], // amm_authority
            [0x03; 32], // amm_open_orders
            [0x05; 32], // pool_coin_token_account (was index 5, now 4)
            [0x06; 32], // pool_pc_token_account (was 6, now 5)
            [0x07; 32], // serum_program
            [0x08; 32], // serum_market
            [0x09; 32], // serum_bids
            [0x0a; 32], // serum_asks
            [0x0b; 32], // serum_event_queue
            [0x0c; 32], // serum_coin_vault
            [0x0d; 32], // serum_pc_vault
            [0x0e; 32], // serum_vault_signer
            [0x0f; 32], // uer_source_token_account
            [0x10; 32], // uer_destination_token_account
            [0x11; 32], // user_source_owner
        ];
        let coin_key_idx = 4 + 2; // post fee_payer + program shift
        let pc_key_idx = 5 + 2;
        let coin_mint = [0xaa; 32];
        let pc_mint = [0xbb; 32];

        let tx = make_tx(&accounts, &[(coin_key_idx as u32, coin_mint), (pc_key_idx as u32, pc_mint)]);
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("legacy decoder must yield InstructionSwap");
        assert_eq!(swap.coin_mint.as_deref(), Some(coin_mint.as_slice()));
        assert_eq!(swap.pc_mint.as_deref(), Some(pc_mint.as_slice()));
    }

    #[test]
    fn unresolved_lookup_keeps_placeholder_with_none_mints() {
        // No pre/post token balances → mint_for misses for both vaults.
        let (accounts, _, _) = make_post_fork_accounts();
        let tx = make_tx(&accounts, &[]);
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("placeholder must still emit");
        assert!(swap.coin_mint.is_none(), "vault must not leak into coin_mint slot when lookup misses");
        assert!(swap.pc_mint.is_none(), "vault must not leak into pc_mint slot when lookup misses");
    }
}
