//! Aldrin AMM V2 initialization carries vaults but omits underlying mints.
//! Only those exact vault accounts may supply transaction metadata.
use super::TokenAccounts;
use crate::{solana::block_view::InstructionView, Candidate, Protocol, Token};
use substreams_solana_idls::aldrin;

pub(super) fn supports(program: &[u8]) -> bool {
    program == aldrin::PROGRAM_ID
}

pub(super) fn decode(ix: &InstructionView<'_>, metadata: &TokenAccounts) -> Option<Candidate> {
    if !supports(ix.program_id().0.as_slice()) {
        return None;
    }
    let aldrin::instructions::AldrinInstruction::Initialize(_) = aldrin::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let accounts = aldrin::accounts::get_initialize_accounts(ix).ok()?;
    let token_program = accounts.token_program.to_bytes();
    let mut tokens = Vec::with_capacity(2);
    for vault in [accounts.base_token_vault, accounts.quote_token_vault] {
        let metadata = metadata.get(&vault.to_bytes())?;
        if metadata.token_program.is_some_and(|program| program != token_program) {
            return None;
        }
        tokens.push(Token {
            is_native: false,
            mint: metadata.mint,
            vault: Some(vault.to_bytes()),
            decimals: metadata.decimals,
            token_program: Some(token_program),
        });
    }
    Some(Candidate {
        protocol: Protocol::Aldrin,
        address: accounts.pool.to_bytes(),
        program_id: aldrin::PROGRAM_ID,
        tokens,
    })
}
