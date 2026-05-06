use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::{
    spl::token_swap::accounts,
    spl::token_swap::instructions::{self, TokenSwapInstruction},
};

use crate::token_mints::TokenMintLookup;

pub(crate) fn handle_instruction(ix: &InstructionView, token_mints: &TokenMintLookup) -> Option<pb::Swap> {
    let program_id = ix.program_id().0;
    let TokenSwapInstruction::Swap {
        amount_in,
        minimum_amount_out,
    } = instructions::unpack(ix.data()).ok()? else {
        return None;
    };

    if amount_in == 0 {
        return None;
    }

    let accounts = accounts::get_swap_accounts(ix).ok()?;
    let input_mint = token_mints.mint_for(accounts.source.as_ref())?;
    let output_mint = token_mints.mint_for(accounts.destination.as_ref())?;

    if input_mint == output_mint {
        return None;
    }

    Some(pb::Swap {
        protocol: pb::Protocol::SplTokenSwap as i32,
        program_id: program_id.to_vec(),
        stack_height: ix.stack_height(),
        amm: program_id.to_vec(),
        amm_pool: accounts.swap_account.to_bytes().to_vec(),
        user: accounts.user_transfer_authority.to_bytes().to_vec(),
        input_mint,
        input_amount: amount_in,
        output_mint,
        output_amount: minimum_amount_out,
    })
}

#[cfg(test)]
mod tests {
    use substreams_solana::{
        base58,
        pb::sf::solana::r#type::v1::{TokenBalance, TransactionStatusMeta, UiTokenAmount},
    };
    use substreams_solana_idls::spl::token_swap;

    use super::*;
    use crate::routed_pool::test_fixture::make_tx;

    const PROGRAM: [u8; 32] = [0x99; 32];
    const SWAP_ACCOUNT: [u8; 32] = [1; 32];
    const AUTHORITY: [u8; 32] = [2; 32];
    const USER: [u8; 32] = [3; 32];
    const SOURCE: [u8; 32] = [4; 32];
    const SWAP_SOURCE: [u8; 32] = [5; 32];
    const SWAP_DESTINATION: [u8; 32] = [6; 32];
    const DESTINATION: [u8; 32] = [7; 32];
    const POOL_MINT: [u8; 32] = [8; 32];
    const FEE_ACCOUNT: [u8; 32] = [9; 32];
    const INPUT_MINT: [u8; 32] = [10; 32];
    const OUTPUT_MINT: [u8; 32] = [11; 32];

    fn swap_data(amount_in: u64, minimum_amount_out: u64) -> Vec<u8> {
        let mut data = vec![instructions::SWAP];
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        data
    }

    fn token_balance(account_index: u32, mint: [u8; 32], amount: u64) -> TokenBalance {
        TokenBalance {
            account_index,
            mint: base58::encode(mint),
            ui_token_amount: Some(UiTokenAmount {
                amount: amount.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn token_mints() -> TokenMintLookup {
        let tx = make_tx(
            PROGRAM,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let mut meta = TransactionStatusMeta::default();
        meta.pre_token_balances = vec![
            token_balance(5, INPUT_MINT, 1_000),
            token_balance(8, OUTPUT_MINT, 100),
        ];
        meta.post_token_balances = vec![
            token_balance(5, INPUT_MINT, 500),
            token_balance(8, OUTPUT_MINT, 100),
        ];
        TokenMintLookup::new(&tx, &meta)
    }

    #[test]
    fn emits_spl_token_swap_for_forked_token_swap_layout() {
        let tx = make_tx(
            PROGRAM,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = token_mints();

        let swap = handle_instruction(&instruction, &token_mints).expect("spl token swap");

        assert_eq!(swap.protocol, pb::Protocol::SplTokenSwap as i32);
        assert_eq!(swap.program_id, PROGRAM.to_vec());
        assert_eq!(swap.amm, PROGRAM.to_vec());
        assert_eq!(swap.amm_pool, SWAP_ACCOUNT.to_vec());
        assert_eq!(swap.user, USER.to_vec());
        assert_eq!(swap.input_mint, INPUT_MINT.to_vec());
        assert_eq!(swap.input_amount, 500);
        assert_eq!(swap.output_mint, OUTPUT_MINT.to_vec());
        assert_eq!(swap.output_amount, 450);
    }

    #[test]
    fn ignores_bad_token_swap_payloads() {
        let tx = make_tx(PROGRAM, &[SWAP_ACCOUNT, AUTHORITY, USER], vec![instructions::SWAP]);
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = token_mints();

        assert!(handle_instruction(&instruction, &token_mints).is_none());
    }

    #[test]
    fn ignores_missing_token_mints() {
        let tx = make_tx(
            PROGRAM,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = TokenMintLookup::new(&tx, &TransactionStatusMeta::default());

        assert!(handle_instruction(&instruction, &token_mints).is_none());
    }

    #[test]
    fn emits_spl_token_swap_for_official_token_swap_program_too() {
        let tx = make_tx(
            token_swap::PROGRAM_ID,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = token_mints();

        assert!(handle_instruction(&instruction, &token_mints).is_some());
    }
}
