use common::solana::parse_program_data;
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
            scoped_program_log(log_message, &raydium::clmm::v3::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let log = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        let (input_amount, output_amount) = if log.zero_for_one {
            (log.amount_0, log.amount_1)
        } else {
            (log.amount_1, log.amount_0)
        };

        Some(pb::Swap {
            protocol: pb::Protocol::RaydiumClmm as i32,
            program_id: raydium::clmm::v3::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::clmm::v3::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state.clone(),
            user: instruction.payer.clone(),
            input_mint: instruction.input_mint.clone(),
            input_amount,
            output_mint: instruction.output_mint.clone(),
            output_amount,
        })
    }
}

struct InstructionSwap {
    stack_height: u32,
    payer: Vec<u8>,
    pool_state: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
}

struct LogSwap {
    amount_0: u64,
    amount_1: u64,
    zero_for_one: bool,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    // `routed_pool::Tracker::observe` only needs the pool address; mints are
    // resolved later in `handle_instruction` when `TokenMintLookup` is in scope.
    decode_instruction(ix, None).map(|s| s.pool_state)
}

fn decode_instruction(ix: &InstructionView, token_mints: Option<&TokenMintLookup>) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::clmm::v3::PROGRAM_ID {
        return None;
    }

    match raydium::clmm::v3::instructions::unpack(ix.data()) {
        Ok(raydium::clmm::v3::instructions::RaydiumClmmInstruction::Swap(_event)) => {
            // Legacy `Swap` only exposes the vault token accounts in its
            // accounts list — the mints are not direct accounts. Resolve them
            // via the tx's pre/post token balances. If `token_mints` is `None`
            // (we're being called from `extract_pool` to populate the routed
            // pools tracker), short-circuit with empty mints; the mints will
            // be filled in correctly when `handle_instruction` re-decodes the
            // same ix with the lookup in scope.
            let accounts = raydium::clmm::v3::accounts::get_swap_accounts(&ix).ok()?;
            let (input_mint, output_mint) = match token_mints {
                Some(lookup) => {
                    let input = lookup.mint_for(accounts.input_vault.to_bytes().as_ref())?;
                    let output = lookup.mint_for(accounts.output_vault.to_bytes().as_ref())?;
                    (input, output)
                }
                None => (Vec::new(), Vec::new()),
            };
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_mint,
                output_mint,
            })
        }
        Ok(raydium::clmm::v3::instructions::RaydiumClmmInstruction::SwapV2(_event)) => {
            let accounts = raydium::clmm::v3::accounts::get_swap_v2_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_mint: accounts.input_vault_mint.to_bytes().to_vec(),
                output_mint: accounts.output_vault_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_program_data(log_message)?;
    match raydium::clmm::v3::events::unpack(data.as_slice()) {
        Ok(raydium::clmm::v3::events::RaydiumClmmEvent::SwapEvent(event)) => Some(LogSwap {
            amount_0: event.amount_0,
            amount_1: event.amount_1,
            zero_for_one: event.zero_for_one,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::base58;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, TokenBalance,
        Transaction, TransactionStatusMeta, UiTokenAmount,
    };

    /// Raydium CLMM v3 SWAP instruction discriminator.
    /// Mirrors the private constant in `substreams_solana_idls::raydium::clmm::v3::instructions`.
    const SWAP_DISC: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
    /// Raydium CLMM v3 SWAP_V2 instruction discriminator.
    const SWAP_V2_DISC: [u8; 8] = [43, 4, 237, 11, 26, 201, 30, 98];

    /// Borsh-encoded `SwapInstruction { amount, other_amount_threshold,
    /// sqrt_price_limit_x64, is_base_input }` body — fields don't matter for
    /// the decoder paths under test, only the discriminator does.
    fn swap_body() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&0u64.to_le_bytes()); // amount
        body.extend_from_slice(&0u64.to_le_bytes()); // other_amount_threshold
        body.extend_from_slice(&0u128.to_le_bytes()); // sqrt_price_limit_x64
        body.push(1); // is_base_input
        body
    }

    /// Build a ConfirmedTransaction with one CLMM v3 instruction.
    /// `accounts` are the per-instruction accounts, in IDL order.
    /// `token_balances` is a list of `(account_index, mint)` pairs to populate
    /// `pre_token_balances` so `TokenMintLookup` can resolve vault → mint.
    fn make_tx(disc: [u8; 8], accounts: &[[u8; 32]], token_balances: &[(u32, [u8; 32])]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = raydium::clmm::v3::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8); // shift past fee_payer (0) and program (1)
        }
        let mut data = disc.to_vec();
        data.extend_from_slice(&swap_body());

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
        // SwapAccounts layout: payer, amm_config, pool_state, input_token_account,
        // output_token_account, input_vault, output_vault, observation_state,
        // token_program, tick_array.
        let payer = [0x01; 32];
        let amm_config = [0x02; 32];
        let pool = [0x03; 32];
        let input_token_acct = [0x04; 32];
        let output_token_acct = [0x05; 32];
        let input_vault = [0x06; 32];
        let output_vault = [0x07; 32];
        let observation = [0x08; 32];
        let token_program = [0x09; 32];
        let tick_array = [0x0a; 32];

        // The actual mints we expect the decoder to surface — different from
        // the vault addresses to prove the lookup did its job.
        let input_mint = [0xaa; 32];
        let output_mint = [0xbb; 32];

        // The vault accounts live at index 2+5 = 7 (input) and 2+6 = 8 (output)
        // in account_keys after make_tx prepends fee_payer + program.
        let token_balances = &[(7, input_mint), (8, output_mint)];

        let tx = make_tx(
            SWAP_DISC,
            &[
                payer,
                amm_config,
                pool,
                input_token_acct,
                output_token_acct,
                input_vault,
                output_vault,
                observation,
                token_program,
                tick_array,
            ],
            token_balances,
        );
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("decoder should yield InstructionSwap");
        assert_eq!(swap.input_mint, input_mint.to_vec(), "input_mint must be the resolved mint, not the vault account");
        assert_eq!(swap.output_mint, output_mint.to_vec(), "output_mint must be the resolved mint, not the vault account");
        assert_eq!(swap.pool_state, pool.to_vec());

        // Sanity: confirm the vault addresses themselves are NOT what we surface.
        assert_ne!(swap.input_mint, input_vault.to_vec(), "regression: vault leaked into mint slot");
        assert_ne!(swap.output_mint, output_vault.to_vec(), "regression: vault leaked into mint slot");
    }

    #[test]
    fn legacy_swap_drops_when_vaults_missing_from_token_balances() {
        // Same instruction shape but no token balances in meta — the lookup
        // can't resolve, so we'd rather drop the swap than write garbage.
        let accounts: Vec<[u8; 32]> = (0u8..10).map(|i| [i + 1; 32]).collect();
        let tx = make_tx(SWAP_DISC, &accounts, &[]);
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        assert!(decode_instruction(&ix, Some(&mints)).is_none());
    }

    #[test]
    fn extract_pool_works_without_token_mints() {
        // routed_pool::Tracker only needs the pool address; mints are absent
        // here because token_mints isn't yet available at the observe call.
        let accounts: Vec<[u8; 32]> = (0u8..10).map(|i| [i + 1; 32]).collect();
        let tx = make_tx(SWAP_DISC, &accounts, &[]);
        let ix = tx.walk_instructions().next().unwrap();

        // pool_state is at IDL index 2, which maps to `accounts[2]` = [3; 32].
        let pool = extract_pool(&ix).expect("extract_pool should succeed without token_mints");
        assert_eq!(pool, [3u8; 32].to_vec());
    }

    #[test]
    fn swap_v2_uses_input_vault_mint_directly() {
        // SwapV2Accounts adds `input_vault_mint` and `output_vault_mint`
        // accounts at the end. We only need to verify the decoder reads them
        // as-is (no vault → mint lookup needed for V2). Layout based on
        // `get_swap_v2_accounts` in the IDL: payer, amm_config, pool_state,
        // input_token_account, output_token_account, input_vault, output_vault,
        // observation_state, token_program, token_program_2022, memo_program,
        // input_vault_mint, output_vault_mint, plus per-call extras.
        let payer = [0x01; 32];
        let amm_config = [0x02; 32];
        let pool = [0x03; 32];
        let input_token_acct = [0x04; 32];
        let output_token_acct = [0x05; 32];
        let input_vault = [0x06; 32];
        let output_vault = [0x07; 32];
        let observation = [0x08; 32];
        let token_prog = [0x09; 32];
        let token_prog_2022 = [0x0a; 32];
        let memo = [0x0b; 32];
        let input_vault_mint = [0xcc; 32];
        let output_vault_mint = [0xdd; 32];

        let tx = make_tx(
            SWAP_V2_DISC,
            &[
                payer,
                amm_config,
                pool,
                input_token_acct,
                output_token_acct,
                input_vault,
                output_vault,
                observation,
                token_prog,
                token_prog_2022,
                memo,
                input_vault_mint,
                output_vault_mint,
            ],
            &[],
        );
        let meta = tx.meta.as_ref().unwrap();
        let mints = TokenMintLookup::new(&tx, meta);
        let ix = tx.walk_instructions().next().unwrap();

        let swap = decode_instruction(&ix, Some(&mints)).expect("v2 decoder should succeed");
        assert_eq!(swap.input_mint, input_vault_mint.to_vec());
        assert_eq!(swap.output_mint, output_vault_mint.to_vec());
    }
}
