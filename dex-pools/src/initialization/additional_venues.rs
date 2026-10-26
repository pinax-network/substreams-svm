//! Pool, curve and market identities from shared initialization layouts.
use crate::{solana::block_view::InstructionView, Candidate, Protocol, Token, NATIVE_SOL_MINT};
use substreams_solana_idls::{byreal::clmm as byreal, darklake, moonshot, pancakeswap, phoenix, plasma, raydium::launchpad, serum};

pub(super) fn supports(program_id: &[u8]) -> bool {
    [
        byreal::PROGRAM_ID,
        pancakeswap::PROGRAM_ID,
        launchpad::PROGRAM_ID,
        phoenix::PROGRAM_ID,
        moonshot::PROGRAM_ID,
        darklake::PROGRAM_ID,
        plasma::PROGRAM_ID,
        serum::PROGRAM_ID,
    ]
    .iter()
    .any(|program| program_id == program)
}

pub(super) fn decode(ix: &InstructionView<'_>) -> Option<Candidate> {
    match ix.program_id().0.as_slice() {
        p if p == byreal::PROGRAM_ID => byreal_pool(ix),
        p if p == pancakeswap::PROGRAM_ID => pancake_pool(ix),
        p if p == launchpad::PROGRAM_ID => launchpad_pool(ix),
        p if p == phoenix::PROGRAM_ID => phoenix_market(ix),
        p if p == moonshot::PROGRAM_ID => moonshot_curve(ix),
        p if p == darklake::PROGRAM_ID => darklake_pool(ix),
        p if p == plasma::PROGRAM_ID => plasma_pool(ix),
        p if p == serum::PROGRAM_ID => serum_market(ix),
        _ => None,
    }
}

fn token(mint: [u8; 32], vault: [u8; 32], program: Option<[u8; 32]>, decimals: Option<u8>) -> Token {
    Token {
        mint,
        vault: Some(vault),
        decimals,
        token_program: program,
        is_native: false,
    }
}

fn byreal_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let byreal::instructions::ByrealClmmInstruction::CreatePool(_) = byreal::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = byreal::accounts::get_create_pool_accounts(ix).ok()?;
    Some(Candidate {
        protocol: Protocol::Byreal,
        address: a.pool_state.to_bytes(),
        program_id: byreal::PROGRAM_ID,
        tokens: vec![
            token(a.token_mint_0.to_bytes(), a.token_vault_0.to_bytes(), Some(a.token_program_0.to_bytes()), None),
            token(a.token_mint_1.to_bytes(), a.token_vault_1.to_bytes(), Some(a.token_program_1.to_bytes()), None),
        ],
    })
}

fn pancake_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let pancakeswap::instructions::PancakeSwapInstruction::CreatePool(_) = pancakeswap::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = pancakeswap::accounts::get_create_pool_accounts(ix).ok()?;
    Some(Candidate {
        protocol: Protocol::Pancakeswap,
        address: a.pool_state.to_bytes(),
        program_id: pancakeswap::PROGRAM_ID,
        tokens: vec![
            token(a.token_mint_0.to_bytes(), a.token_vault_0.to_bytes(), Some(a.token_program_0.to_bytes()), None),
            token(a.token_mint_1.to_bytes(), a.token_vault_1.to_bytes(), Some(a.token_program_1.to_bytes()), None),
        ],
    })
}

fn launchpad_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    use launchpad::instructions::RaydiumLaunchpadInstruction;
    let (pool, base, quote, base_vault, quote_vault, base_program, quote_program, program, decimals) = match launchpad::instructions::unpack(ix.data()).ok()? {
        RaydiumLaunchpadInstruction::Initialize(init) => {
            let a = launchpad::accounts::get_initialize_accounts(ix).ok()?;
            (
                a.pool_state,
                a.base_mint,
                a.quote_mint,
                a.base_vault,
                a.quote_vault,
                a.base_token_program,
                a.quote_token_program,
                a.program,
                init.base_mint_param.decimals,
            )
        }
        RaydiumLaunchpadInstruction::InitializeV2(init) => {
            let a = launchpad::accounts::get_initialize_v2_accounts(ix).ok()?;
            (
                a.pool_state,
                a.base_mint,
                a.quote_mint,
                a.base_vault,
                a.quote_vault,
                a.base_token_program,
                a.quote_token_program,
                a.program,
                init.base_mint_param.decimals,
            )
        }
        RaydiumLaunchpadInstruction::InitializeWithToken2022(init) => {
            let a = launchpad::accounts::get_initialize_with_token_2022_accounts(ix).ok()?;
            (
                a.pool_state,
                a.base_mint,
                a.quote_mint,
                a.base_vault,
                a.quote_vault,
                a.base_token_program,
                a.quote_token_program,
                a.program,
                init.base_mint_param.decimals,
            )
        }
        _ => return None,
    };
    if program.to_bytes() != launchpad::PROGRAM_ID {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::RaydiumLaunchpad,
        address: pool.to_bytes(),
        program_id: launchpad::PROGRAM_ID,
        tokens: vec![
            token(base.to_bytes(), base_vault.to_bytes(), Some(base_program.to_bytes()), Some(decimals)),
            token(quote.to_bytes(), quote_vault.to_bytes(), Some(quote_program.to_bytes()), None),
        ],
    })
}

fn phoenix_market(ix: &InstructionView<'_>) -> Option<Candidate> {
    let phoenix::instructions::PhonenixInstruction::InitializeMarket(_) = phoenix::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = phoenix::accounts::get_initialize_market_accounts(ix).ok()?;
    if a.phoenix_program.to_bytes() != phoenix::PROGRAM_ID {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::Phoenix,
        address: a.market.to_bytes(),
        program_id: phoenix::PROGRAM_ID,
        tokens: vec![
            token(a.base_mint.to_bytes(), a.base_vault.to_bytes(), Some(a.token_program.to_bytes()), None),
            token(a.quote_mint.to_bytes(), a.quote_vault.to_bytes(), Some(a.token_program.to_bytes()), None),
        ],
    })
}

fn moonshot_curve(ix: &InstructionView<'_>) -> Option<Candidate> {
    let moonshot::instructions::MoonshotInstruction::TokenMint(init) = moonshot::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    // The shared IDL's Currency enum establishes SOL as the supported quote.
    // Do not reinterpret a future currency value as native SOL.
    if init.collateral_currency != moonshot::accounts::Currency::Sol as u8 {
        return None;
    }
    let a = moonshot::accounts::get_token_mint_accounts(ix).ok()?;
    Some(Candidate {
        protocol: Protocol::Moonshot,
        address: a.curve_account.to_bytes(),
        program_id: moonshot::PROGRAM_ID,
        tokens: vec![
            token(
                a.mint.to_bytes(),
                a.curve_token_account.to_bytes(),
                Some(a.token_program.to_bytes()),
                Some(init.decimals),
            ),
            Token {
                mint: NATIVE_SOL_MINT,
                is_native: true,
                vault: None,
                decimals: Some(9),
                token_program: None,
            },
        ],
    })
}

fn darklake_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let darklake::instructions::DarklakeInstruction::InitializePool(_) = darklake::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = darklake::accounts::get_initialize_pool_accounts(ix).ok()?;
    Some(Candidate {
        protocol: Protocol::Darklake,
        address: a.pool.to_bytes(),
        program_id: darklake::PROGRAM_ID,
        // The additional WSOL reserve/LP mint are not the pool's X/Y assets.
        tokens: vec![
            token(
                a.token_mint_x.to_bytes(),
                a.pool_token_reserve_x.to_bytes(),
                Some(a.token_mint_x_program.to_bytes()),
                None,
            ),
            token(
                a.token_mint_y.to_bytes(),
                a.pool_token_reserve_y.to_bytes(),
                Some(a.token_mint_y_program.to_bytes()),
                None,
            ),
        ],
    })
}

fn plasma_pool(ix: &InstructionView<'_>) -> Option<Candidate> {
    let plasma::instructions::PlasmaInstruction::InitializePool(_) = plasma::instructions::unpack(ix.data()).ok()? else {
        return None;
    };
    let a = plasma::accounts::get_initialize_pool_accounts(ix).ok()?;
    if a.plasma_program.to_bytes() != plasma::PROGRAM_ID {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::Plasma,
        address: a.pool.to_bytes(),
        program_id: plasma::PROGRAM_ID,
        tokens: vec![
            token(a.base_mint.to_bytes(), a.base_vault.to_bytes(), Some(a.token_program.to_bytes()), None),
            token(a.quote_mint.to_bytes(), a.quote_vault.to_bytes(), Some(a.token_program.to_bytes()), None),
        ],
    })
}

fn serum_market(ix: &InstructionView<'_>) -> Option<Candidate> {
    serum::instructions::unpack_initialize_market(ix.data()).ok()?;
    // The shared helper covers the nine identity accounts. The canonical
    // initializer additionally requires rent, before optional authorities.
    if ix.accounts().len() < 10 {
        return None;
    }
    let a = serum::accounts::get_initialize_market_accounts(ix).ok()?;
    Some(Candidate {
        protocol: Protocol::Serum,
        address: a.market_to_initialize.to_bytes(),
        program_id: serum::PROGRAM_ID,
        // No token-program account or mint decimals appear in this initializer.
        tokens: vec![
            token(a.coin_currency_mint.to_bytes(), a.spl_token_account_coin.to_bytes(), None, None),
            token(a.price_currency_mint.to_bytes(), a.spl_token_account_price.to_bytes(), None, None),
        ],
    })
}
