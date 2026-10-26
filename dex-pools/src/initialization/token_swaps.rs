//! SPL Token Swap and Saros initialization omit underlying mints. Resolve only
//! their exact vault accounts from supplied transaction token-account metadata.
use super::TokenAccounts;
use crate::{solana::block_view::InstructionView, Candidate, Protocol, Token};
use substreams_solana_idls::{saros, spl::token_swap, stabble};

pub(super) fn supports(program: &[u8]) -> bool {
    program == token_swap::PROGRAM_ID || program == saros::PROGRAM_ID || program == stabble::PROGRAM_ID
}

pub(super) fn needs_token_accounts(program: &[u8]) -> bool {
    program == token_swap::PROGRAM_ID || program == saros::PROGRAM_ID
}

pub(super) fn decode(ix: &InstructionView<'_>, metadata: &TokenAccounts) -> Option<Candidate> {
    let program = ix.program_id().0.as_slice();
    let (protocol, pool, vaults, token_program) = if program == token_swap::PROGRAM_ID {
        token_swap::instructions::unpack_initialize(ix.data()).ok()?;
        let a = token_swap::accounts::get_initialize_accounts(ix).ok()?;
        (Protocol::SplTokenSwap, a.swap_account, [a.token_a, a.token_b], a.token_program)
    } else if program == saros::PROGRAM_ID {
        let saros::instructions::SarosInstruction::Initialize(_) = saros::instructions::unpack(ix.data()).ok()? else {
            return None;
        };
        let a = saros::accounts::get_initialize_accounts(ix).ok()?;
        (Protocol::Saros, a.swap_account, [a.token_a, a.token_b], a.token_program)
    } else if program == stabble::PROGRAM_ID {
        return weighted_pool(ix);
    } else {
        return None;
    };
    let mut tokens = Vec::with_capacity(2);
    for vault in vaults {
        let meta = metadata.get(&vault.to_bytes())?;
        if meta.token_program.is_some_and(|program| program != token_program.to_bytes()) {
            return None;
        }
        tokens.push(Token {
            mint: meta.mint,
            vault: Some(vault.to_bytes()),
            decimals: meta.decimals,
            token_program: Some(token_program.to_bytes()),
            is_native: false,
        });
    }
    Some(Candidate {
        protocol,
        address: pool.to_bytes(),
        program_id: program.try_into().ok()?,
        tokens,
    })
}

fn weighted_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let stabble::instructions::StabbleInstruction::Initialize(init) = stabble::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = stabble::accounts::get_initialize_accounts(ix).ok()?;
    let mints = stabble::accounts::get_initialize_mints(ix).ok()?;
    if mints.len() != init.weights.len() || mints.len() != init.max_caps.len() {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::Stabble,
        address: a.pool.to_bytes(),
        program_id: stabble::PROGRAM_ID,
        // Remaining accounts are underlying mints, in weight order. The fixed
        // prefix's mint is the LP mint; its vault is program state, not a token
        // custody account. Neither belongs in the underlying asset list.
        tokens: mints
            .into_iter()
            .map(|mint| Token {
                mint: mint.to_bytes(),
                vault: None,
                decimals: None,
                token_program: None,
                is_native: false,
            })
            .collect(),
    })
}
