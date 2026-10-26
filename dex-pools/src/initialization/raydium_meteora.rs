//! Pool identities from shared IDL initialization instructions and accounts.
//! Token order is coin/pc, 0/1, A/B, or X/Y as defined by each protocol.
use crate::{solana::block_view::InstructionView, Candidate, Protocol, Token};
use substreams_solana_idls::{
    meteora::{amm, daam, dlmm},
    raydium::{amm::v4 as amm_v4, clmm::v3 as clmm, cpmm},
};

pub(super) fn supports(program_id: &[u8]) -> bool {
    [
        amm_v4::PROGRAM_ID,
        clmm::PROGRAM_ID,
        cpmm::PROGRAM_ID,
        amm::PROGRAM_ID,
        dlmm::PROGRAM_ID,
        daam::PROGRAM_ID,
    ]
    .iter()
    .any(|program| program_id == program)
}

pub(super) fn decode(ix: &InstructionView<'_>) -> Option<Candidate> {
    let program = ix.program_id();
    match program.0.as_slice() {
        p if p == amm_v4::PROGRAM_ID => raydium_amm(ix),
        p if p == clmm::PROGRAM_ID => raydium_clmm(ix),
        p if p == cpmm::PROGRAM_ID => raydium_cpmm(ix),
        p if p == amm::PROGRAM_ID => meteora_amm(ix),
        p if p == dlmm::PROGRAM_ID => meteora_dlmm(ix),
        p if p == daam::PROGRAM_ID => meteora_daam(ix),
        _ => None,
    }
}

fn token(mint: [u8; 32], vault: Option<[u8; 32]>, token_program: [u8; 32]) -> Token {
    Token {
        mint,
        is_native: false,
        vault,
        decimals: None,
        token_program: Some(token_program),
    }
}

#[allow(deprecated)] // Historical Initialize remains meaningful creation evidence.
fn raydium_amm(ix: &InstructionView<'_>) -> Option<Candidate> {
    use amm_v4::instructions::{self as instructions, RaydiumV4Instruction};
    // The shared AMM decoder tolerates trailing swap bytes and slices its swap
    // payload. Only dispatch initializers here, including for malformed input.
    if !matches!(ix.data().first(), Some(&instructions::INITIALIZE) | Some(&instructions::INITIALIZE2)) {
        return None;
    }
    let (pool, mint_a, mint_b, vault_a, vault_b, program) = match instructions::unpack(ix.data()).ok()? {
        RaydiumV4Instruction::Initialize(_) => {
            let a = amm_v4::accounts::get_initialize_accounts(ix).ok()?;
            (
                a.amm,
                a.coin_mint_address,
                a.pc_mint_address,
                a.pool_coin_token_account,
                a.pool_pc_token_account,
                a.token_program,
            )
        }
        RaydiumV4Instruction::Initialize2(_) => {
            let a = amm_v4::accounts::get_initialize2_accounts(ix).ok()?;
            (
                a.amm,
                a.coin_mint,
                a.pc_mint,
                a.pool_coin_token_account,
                a.pool_pc_token_account,
                a.token_program,
            )
        }
        _ => return None,
    };
    Some(Candidate {
        protocol: Protocol::RaydiumAmmV4,
        address: pool.to_bytes(),
        program_id: amm_v4::PROGRAM_ID,
        tokens: vec![
            token(mint_a.to_bytes(), Some(vault_a.to_bytes()), program.to_bytes()),
            token(mint_b.to_bytes(), Some(vault_b.to_bytes()), program.to_bytes()),
        ],
    })
}

fn raydium_clmm(ix: &InstructionView<'_>) -> Option<Candidate> {
    use clmm::instructions::RaydiumClmmInstruction;
    let (pool, mint_a, mint_b, vault_a, vault_b, program_a, program_b) = match clmm::instructions::unpack(ix.data()).ok()? {
        RaydiumClmmInstruction::CreatePool(_) => {
            let a = clmm::accounts::get_create_pool_accounts(ix).ok()?;
            (
                a.pool_state,
                a.token_mint_0,
                a.token_mint_1,
                a.token_vault_0,
                a.token_vault_1,
                a.token_program_0,
                a.token_program_1,
            )
        }
        RaydiumClmmInstruction::CreateCustomizablePool(_) => {
            let a = clmm::accounts::get_create_customizable_pool_accounts(ix).ok()?;
            (
                a.pool_state,
                a.token_mint_0,
                a.token_mint_1,
                a.token_vault_0,
                a.token_vault_1,
                a.token_program_0,
                a.token_program_1,
            )
        }
        RaydiumClmmInstruction::CreateCustomizablePoolV2(_) => {
            let a = clmm::accounts::get_create_customizable_pool_v2_accounts(ix).ok()?;
            (
                a.pool_state,
                a.token_mint_0,
                a.token_mint_1,
                a.token_vault_0,
                a.token_vault_1,
                a.token_program_0,
                a.token_program_1,
            )
        }
        RaydiumClmmInstruction::CreatePermissionedPool(_) => {
            let a = clmm::accounts::get_create_permissioned_pool_accounts(ix).ok()?;
            (
                a.pool_state,
                a.token_mint_0,
                a.token_mint_1,
                a.token_vault_0,
                a.token_vault_1,
                a.token_program_0,
                a.token_program_1,
            )
        }
        _ => return None,
    };
    Some(Candidate {
        protocol: Protocol::RaydiumClmm,
        address: pool.to_bytes(),
        program_id: clmm::PROGRAM_ID,
        tokens: vec![
            token(mint_a.to_bytes(), Some(vault_a.to_bytes()), program_a.to_bytes()),
            token(mint_b.to_bytes(), Some(vault_b.to_bytes()), program_b.to_bytes()),
        ],
    })
}

fn raydium_cpmm(ix: &InstructionView<'_>) -> Option<Candidate> {
    use cpmm::instructions::RaydiumCpmmInstruction;
    let (pool, mint_a, mint_b, vault_a, vault_b, program_a, program_b) = match cpmm::instructions::unpack(ix.data()).ok()? {
        RaydiumCpmmInstruction::Initialize(_) => {
            let a = cpmm::accounts::get_initialize_accounts(ix).ok()?;
            (
                a.pool_state,
                a.token_0_mint,
                a.token_1_mint,
                a.token_0_vault,
                a.token_1_vault,
                a.token_0_program,
                a.token_1_program,
            )
        }
        RaydiumCpmmInstruction::InitializeWithPermission(_) => {
            let a = cpmm::accounts::get_initialize_with_permission_accounts(ix).ok()?;
            (
                a.pool_state,
                a.token_0_mint,
                a.token_1_mint,
                a.token_0_vault,
                a.token_1_vault,
                a.token_0_program,
                a.token_1_program,
            )
        }
        _ => return None,
    };
    Some(Candidate {
        protocol: Protocol::RaydiumCpmm,
        address: pool.to_bytes(),
        program_id: cpmm::PROGRAM_ID,
        tokens: vec![
            token(mint_a.to_bytes(), Some(vault_a.to_bytes()), program_a.to_bytes()),
            token(mint_b.to_bytes(), Some(vault_b.to_bytes()), program_b.to_bytes()),
        ],
    })
}

fn meteora_amm(ix: &InstructionView<'_>) -> Option<Candidate> {
    use amm::instructions::AmmInstruction;
    let (pool, mint_a, mint_b, vault_a, vault_b, program) = match amm::instructions::unpack(ix.data()).ok()? {
        AmmInstruction::InitializePermissionedPool(_) => {
            let a = amm::accounts::get_initialize_permissioned_pool_accounts(ix).ok()?;
            // a_vault/b_vault are vault-program state accounts, not token
            // custody accounts. This initializer does not supply token vaults.
            (a.pool, a.token_a_mint, a.token_b_mint, None, None, a.token_program)
        }
        AmmInstruction::InitializePermissionlessPool(_) => {
            let a = amm::accounts::get_initialize_permissionless_pool_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                Some(a.a_token_vault),
                Some(a.b_token_vault),
                a.token_program,
            )
        }
        AmmInstruction::InitializePermissionlessPoolWithFeeTier(_) => {
            let a = amm::accounts::get_initialize_permissionless_pool_with_fee_tier_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                Some(a.a_token_vault),
                Some(a.b_token_vault),
                a.token_program,
            )
        }
        AmmInstruction::InitializePermissionlessConstantProductPoolWithConfig(_) => {
            let a = amm::accounts::get_initialize_permissionless_constant_product_pool_with_config_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                Some(a.a_token_vault),
                Some(a.b_token_vault),
                a.token_program,
            )
        }
        AmmInstruction::InitializePermissionlessConstantProductPoolWithConfig2(_) => {
            let a = amm::accounts::get_initialize_permissionless_constant_product_pool_with_config2_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                Some(a.a_token_vault),
                Some(a.b_token_vault),
                a.token_program,
            )
        }
        AmmInstruction::InitializeCustomizablePermissionlessConstantProductPool(_) => {
            let a = amm::accounts::get_initialize_customizable_permissionless_constant_product_pool_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                Some(a.a_token_vault),
                Some(a.b_token_vault),
                a.token_program,
            )
        }
        _ => return None,
    };
    Some(Candidate {
        protocol: Protocol::MeteoraAmm,
        address: pool.to_bytes(),
        program_id: amm::PROGRAM_ID,
        tokens: vec![
            token(mint_a.to_bytes(), vault_a.map(|vault| vault.to_bytes()), program.to_bytes()),
            token(mint_b.to_bytes(), vault_b.map(|vault| vault.to_bytes()), program.to_bytes()),
        ],
    })
}

fn meteora_dlmm(ix: &InstructionView<'_>) -> Option<Candidate> {
    use dlmm::instructions::MeteoraDlmmInstruction;
    let (pool, mint_a, mint_b, vault_a, vault_b, program_a, program_b, program) = match dlmm::instructions::unpack(ix.data()).ok()? {
        MeteoraDlmmInstruction::InitializeCustomizablePermissionlessLbPair(_) => {
            let a = dlmm::accounts::get_initialize_customizable_permissionless_lb_pair_accounts(ix).ok()?;
            (
                a.lb_pair,
                a.token_mint_x,
                a.token_mint_y,
                a.reserve_x,
                a.reserve_y,
                a.token_program,
                a.token_program,
                a.program,
            )
        }
        MeteoraDlmmInstruction::InitializeCustomizablePermissionlessLbPair2(_) => {
            let a = dlmm::accounts::get_initialize_customizable_permissionless_lb_pair2_accounts(ix).ok()?;
            (
                a.lb_pair,
                a.token_mint_x,
                a.token_mint_y,
                a.reserve_x,
                a.reserve_y,
                a.token_program_x,
                a.token_program_y,
                a.program,
            )
        }
        MeteoraDlmmInstruction::InitializeLbPair(_) => {
            let a = dlmm::accounts::get_initialize_lb_pair_accounts(ix).ok()?;
            (
                a.lb_pair,
                a.token_mint_x,
                a.token_mint_y,
                a.reserve_x,
                a.reserve_y,
                a.token_program,
                a.token_program,
                a.program,
            )
        }
        MeteoraDlmmInstruction::InitializeLbPair2(_) => {
            let a = dlmm::accounts::get_initialize_lb_pair2_accounts(ix).ok()?;
            (
                a.lb_pair,
                a.token_mint_x,
                a.token_mint_y,
                a.reserve_x,
                a.reserve_y,
                a.token_program_x,
                a.token_program_y,
                a.program,
            )
        }
        MeteoraDlmmInstruction::InitializePermissionLbPair(_) => {
            let a = dlmm::accounts::get_initialize_permission_lb_pair_accounts(ix).ok()?;
            (
                a.lb_pair,
                a.token_mint_x,
                a.token_mint_y,
                a.reserve_x,
                a.reserve_y,
                a.token_program_x,
                a.token_program_y,
                a.program,
            )
        }
        _ => return None,
    };
    if program.to_bytes() != dlmm::PROGRAM_ID {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::MeteoraDlmm,
        address: pool.to_bytes(),
        program_id: dlmm::PROGRAM_ID,
        tokens: vec![
            token(mint_a.to_bytes(), Some(vault_a.to_bytes()), program_a.to_bytes()),
            token(mint_b.to_bytes(), Some(vault_b.to_bytes()), program_b.to_bytes()),
        ],
    })
}

fn meteora_daam(ix: &InstructionView<'_>) -> Option<Candidate> {
    use daam::instructions::MeteoraDammInstruction;
    let (pool, mint_a, mint_b, vault_a, vault_b, program_a, program_b, program) = match daam::instructions::unpack(ix.data()).ok()? {
        MeteoraDammInstruction::InitializeCustomizablePool(_) => {
            let a = daam::accounts::get_initialize_customizable_pool_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                a.token_a_vault,
                a.token_b_vault,
                a.token_a_program,
                a.token_b_program,
                a.program,
            )
        }
        MeteoraDammInstruction::InitializePool(_) => {
            let a = daam::accounts::get_initialize_pool_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                a.token_a_vault,
                a.token_b_vault,
                a.token_a_program,
                a.token_b_program,
                a.program,
            )
        }
        MeteoraDammInstruction::InitializePoolWithDynamicConfig(_) => {
            let a = daam::accounts::get_initialize_pool_with_dynamic_config_accounts(ix).ok()?;
            (
                a.pool,
                a.token_a_mint,
                a.token_b_mint,
                a.token_a_vault,
                a.token_b_vault,
                a.token_a_program,
                a.token_b_program,
                a.program,
            )
        }
        _ => return None,
    };
    if program.to_bytes() != daam::PROGRAM_ID {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::MeteoraDaam,
        address: pool.to_bytes(),
        program_id: daam::PROGRAM_ID,
        tokens: vec![
            token(mint_a.to_bytes(), Some(vault_a.to_bytes()), program_a.to_bytes()),
            token(mint_b.to_bytes(), Some(vault_b.to_bytes()), program_b.to_bytes()),
        ],
    })
}
