//! Creation candidates from shared typed instruction and account decoders.
//! Execution success and all enclosing invocation outcomes are checked by the
//! projector; these functions do not themselves establish commitment.
use crate::{solana::block_view::InstructionView, Candidate, Protocol, Token, NATIVE_SOL_MINT};
use substreams_solana_idls::{
    bonkswap, boop, dumpfun, openbook,
    pumpfun::bonding_curve as pump,
    spl::{token, token_2022},
};

pub(super) fn supports(program_id: &[u8]) -> bool {
    program_id == bonkswap::PROGRAM_ID
        || program_id == pump::PROGRAM_ID
        || program_id == openbook::PROGRAM_ID
        || program_id == boop::PROGRAM_ID
        || program_id == dumpfun::PROGRAM_ID
}

pub(super) fn decode(ix: &InstructionView<'_>) -> Option<Candidate> {
    let program = ix.program_id().0.as_slice();
    if program == bonkswap::PROGRAM_ID {
        bonkswap_pool(ix)
    } else if program == pump::PROGRAM_ID {
        pump_bonding_curve(ix)
    } else if program == openbook::PROGRAM_ID {
        openbook_market(ix)
    } else if program == boop::PROGRAM_ID {
        boop_curve(ix)
    } else if program == dumpfun::PROGRAM_ID {
        dumpfun_pool(ix)
    } else {
        None
    }
}

fn bonkswap_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let bonkswap::instructions::BonkSwapInstruction::CreatePool(_) = bonkswap::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let accounts = bonkswap::accounts::get_create_pool_accounts(ix).ok()?;
    Some(Candidate {
        protocol: Protocol::Bonkswap,
        address: accounts.pool.to_bytes(),
        program_id: bonkswap::PROGRAM_ID,
        tokens: vec![
            Token {
                mint: accounts.token_x.to_bytes(),
                vault: Some(accounts.pool_x_account.to_bytes()),
                decimals: None,
                token_program: Some(accounts.token_program.to_bytes()),
                is_native: false,
            },
            Token {
                mint: accounts.token_y.to_bytes(),
                vault: Some(accounts.pool_y_account.to_bytes()),
                decimals: None,
                token_program: Some(accounts.token_program.to_bytes()),
                is_native: false,
            },
        ],
    })
}

fn pump_bonding_curve(ix: &InstructionView<'_>) -> Option<Candidate> {
    // Legacy create is Tokenkeg/native-SOL. Current create_v2 has a different
    // Token2022 account layout and may choose a token quote in remaining
    // accounts. Keep both layouts and their quote semantics explicit.
    let (mint, vault, pool, program, quote) = match pump::instructions::unpack(ix.data()).ok()? {
        pump::instructions::PumpFunInstruction::Create(_) => {
            let a = pump::accounts::get_create_accounts(ix).ok()?;
            if a.program.to_bytes() != pump::PROGRAM_ID || a.token_program.to_bytes() != token::PROGRAM_ID {
                return None;
            }
            (a.mint, a.curve, a.bonding_curve, a.token_program, native_sol())
        }
        pump::instructions::PumpFunInstruction::CreateV2(_) => {
            let a = pump::accounts::get_create_v2_accounts(ix).ok()?;
            if a.program.to_bytes() != pump::PROGRAM_ID || a.token_program.to_bytes() != token_2022::PROGRAM_ID {
                return None;
            }
            let quote = match pump::accounts::get_create_v2_quote_accounts(ix).ok()? {
                None => native_sol(),
                Some(q) => Token {
                    mint: q.quote_mint.to_bytes(),
                    vault: Some(q.associated_quote_bonding_curve.to_bytes()),
                    token_program: Some(q.quote_token_program.to_bytes()),
                    decimals: None,
                    is_native: false,
                },
            };
            (a.mint, a.associated_bonding_curve, a.bonding_curve, a.token_program, quote)
        }
        _ => return None,
    };
    Some(Candidate {
        protocol: Protocol::PumpfunBondingCurve,
        address: pool.to_bytes(),
        program_id: pump::PROGRAM_ID,
        tokens: vec![
            Token {
                mint: mint.to_bytes(),
                vault: Some(vault.to_bytes()),
                decimals: None,
                token_program: Some(program.to_bytes()),
                is_native: false,
            },
            quote,
        ],
    })
}

fn native_sol() -> Token {
    Token {
        mint: NATIVE_SOL_MINT,
        vault: None,
        decimals: Some(9),
        token_program: None,
        is_native: true,
    }
}

fn boop_curve(ix: &InstructionView<'_>) -> Option<Candidate> {
    use boop::instructions::BoopInstruction;
    let a = match boop::instructions::unpack(ix.data()).ok()? {
        BoopInstruction::DeployBondingCurve(_) => boop::accounts::get_deploy_bonding_curve_accounts(ix).ok()?,
        BoopInstruction::DeployBondingCurveFallback(_) => boop::accounts::get_deploy_bonding_curve_fallback_accounts(ix).ok()?,
        _ => return None,
    };
    // The declared program's on-chain IDL explicitly has native-SOL reserves;
    // the SOL vault is not represented as an SPL token custody account.
    Some(Candidate {
        protocol: Protocol::Boop,
        address: a.bonding_curve.to_bytes(),
        program_id: boop::PROGRAM_ID,
        tokens: vec![
            Token {
                mint: a.mint.to_bytes(),
                vault: Some(a.bonding_curve_vault.to_bytes()),
                decimals: None,
                token_program: Some(a.token_program.to_bytes()),
                is_native: false,
            },
            native_sol(),
        ],
    })
}

fn dumpfun_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let dumpfun::instructions::DumpfunInstruction::Mint(_) = dumpfun::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = dumpfun::accounts::get_mint_accounts(ix).ok()?;
    // Mint's virtual_sol_reserve and the declared program's SOL trade
    // instructions establish the quote asset. No distinct SPL quote vault.
    Some(Candidate {
        protocol: Protocol::Dumpfun,
        address: a.liquidity_pool.to_bytes(),
        program_id: dumpfun::PROGRAM_ID,
        tokens: vec![
            Token {
                mint: a.mint_account.to_bytes(),
                vault: Some(a.liquidity_pool_token_account.to_bytes()),
                decimals: None,
                token_program: Some(a.token_program.to_bytes()),
                is_native: false,
            },
            native_sol(),
        ],
    })
}

fn openbook_market(ix: &InstructionView<'_>) -> Option<Candidate> {
    // The shared IDL recognizes only the discriminator for CreateMarket. This
    // is therefore a candidate from its typed account layout, not argument
    // validation. Only an exactly proved successful initialization invocation
    // may promote it to a Pool; the raw API cannot establish that guarantee.
    let openbook::instructions::OpenbookInstruction::CreateMarket = openbook::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let accounts = openbook::accounts::get_create_market_accounts(ix).ok()?;
    if accounts.program.to_bytes() != openbook::PROGRAM_ID {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::Openbook,
        // An order-book market is the stable identity; it is not an AMM pool.
        address: accounts.market.to_bytes(),
        program_id: openbook::PROGRAM_ID,
        tokens: vec![
            Token {
                mint: accounts.base_mint.to_bytes(),
                vault: Some(accounts.market_base_vault.to_bytes()),
                decimals: None,
                token_program: Some(accounts.token_program.to_bytes()),
                is_native: false,
            },
            Token {
                mint: accounts.quote_mint.to_bytes(),
                vault: Some(accounts.market_quote_vault.to_bytes()),
                decimals: None,
                token_program: Some(accounts.token_program.to_bytes()),
                is_native: false,
            },
        ],
    })
}
