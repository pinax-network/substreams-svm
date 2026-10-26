use crate::{solana, Candidate, Protocol, Token};
use buffa::MessageField;
use solana::{block_view::InstructionView, pb::sf::solana::r#type::v1 as pb};
use substreams_solana_idls::{
    orca::whirlpool as orca,
    pumpfun::amm as pump,
    spl::{token, token_2022},
};

/// Decode a matched initialization and binary event from borrowed bytes.
///
/// This is transport-independent: callers with Prost `Bytes`, `Vec<u8>`, or
/// Buffa models borrow their resolved instruction accounts and data directly.
/// No block is serialized. At most 32 account keys and 2048 instruction bytes
/// are copied into a tiny instruction view to reuse the IDL's typed accounts.
///
/// The returned [`Candidate`] is **unverified**. This function cannot establish
/// program ownership of event bytes, transaction success, or CPI ancestry. Use
/// [`crate::project`] for complete block evidence admission.
/// Unsupported, malformed, mismatched and oversized inputs return `None`.
pub fn decode_pool_creation(program_id: &[u8], instruction_data: &[u8], accounts: &[&[u8]], event_data: &[u8]) -> Option<Candidate> {
    if program_id != pump::PROGRAM_ID && program_id != orca::PROGRAM_ID {
        return None;
    }
    if accounts.len() > 32 || accounts.iter().any(|account| account.len() != 32) || instruction_data.len() > 2048 || event_data.len() > 2048 {
        return None;
    }
    let mut account_keys = Vec::with_capacity(accounts.len() + 1);
    account_keys.push(program_id.to_vec());
    account_keys.extend(accounts.iter().map(|account| account.to_vec()));
    let tx = pb::ConfirmedTransaction {
        transaction: MessageField::some(pb::Transaction {
            message: MessageField::some(pb::Message {
                account_keys,
                instructions: vec![pb::CompiledInstruction {
                    program_id_index: 0,
                    accounts: (1..=accounts.len() as u8).collect(),
                    data: instruction_data.to_vec(),
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let instruction = tx.walk_instructions().next()?;
    from_view(&instruction, event_data)
}

pub(crate) fn from_view(ix: &InstructionView<'_>, event: &[u8]) -> Option<Candidate> {
    if ix.data().len() > 2048 || event.len() > 2048 || ix.accounts().len() > 32 {
        return None;
    }
    let candidate = if ix.program_id().0.as_slice() == pump::PROGRAM_ID {
        pump_pool(ix, event)
    } else if ix.program_id().0.as_slice() == orca::PROGRAM_ID {
        orca_pool(ix, event)
    } else {
        None
    }?;
    if candidate.address == [0; 32] || candidate.address == candidate.program_id || candidate.tokens[0].mint == candidate.tokens[1].mint {
        return None;
    }
    for token in &candidate.tokens {
        if token.mint == [0; 32] || token.mint == candidate.address || token.vault == Some([0; 32]) {
            return None;
        }
        if !token
            .token_program
            .is_some_and(|program| program == token::PROGRAM_ID || program == token_2022::PROGRAM_ID)
        {
            return None;
        }
    }
    if candidate.tokens[0].vault.is_some() && candidate.tokens[0].vault == candidate.tokens[1].vault {
        return None;
    }
    Some(candidate)
}

fn pump_pool(ix: &InstructionView<'_>, data: &[u8]) -> Option<Candidate> {
    use pump::{events::PumpFunAmmEvent, instructions::PumpFunAmmInstruction};
    let (init, event) = match (pump::instructions::unpack(ix.data()).ok()?, pump::events::unpack(data).ok()?) {
        (PumpFunAmmInstruction::CreatePoolV2(init), PumpFunAmmEvent::CreatePoolEventV2(event)) => (init, event),
        (PumpFunAmmInstruction::CreatePoolV3(init), PumpFunAmmEvent::CreatePoolEventV3(event)) => {
            if init.is_mayhem_mode != event.is_mayhem_mode
                || init.creator_fee_bps.0 != event.creator_fee_bps
                || init.can_edit_creator_fee.0 != event.can_edit_creator_fee
                || init.is_holder_reward.0 != event.is_holder_reward
            {
                return None;
            }
            (init.base, event.base)
        }
        _ => return None,
    };
    let accounts = pump::accounts::get_create_pool_accounts(ix).ok()?;
    if event.pool != accounts.pool
        || event.base_mint != accounts.base_mint
        || event.quote_mint != accounts.quote_mint
        || event.creator != accounts.creator
        || event.lp_mint != accounts.lp_mint
        || event.user_base_token_account != accounts.user_base_token_account
        || event.user_quote_token_account != accounts.user_quote_token_account
        || event.index != init.index
        || event.base_amount_in != init.base_amount_in
        || event.quote_amount_in != init.quote_amount_in
        || event.coin_creator != init.coin_creator
        || accounts.program.to_bytes() != pump::PROGRAM_ID
    {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::PumpfunAmm,
        address: event.pool.to_bytes(),
        program_id: pump::PROGRAM_ID,
        tokens: [
            Token {
                mint: event.base_mint.to_bytes(),
                vault: Some(accounts.pool_base_token_account.to_bytes()),
                decimals: Some(event.base_mint_decimals),
                token_program: Some(accounts.base_token_program.to_bytes()),
            },
            Token {
                mint: event.quote_mint.to_bytes(),
                vault: Some(accounts.pool_quote_token_account.to_bytes()),
                decimals: Some(event.quote_mint_decimals),
                token_program: Some(accounts.quote_token_program.to_bytes()),
            },
        ],
    })
}

fn orca_pool(ix: &InstructionView<'_>, data: &[u8]) -> Option<Candidate> {
    let orca::events::WhirlpoolEvent::PoolInitialized(event) = orca::events::parse_event(data).ok()? else {
        return None;
    };
    let (pool, config, mint_a, mint_b, vault_a, vault_b, program_a, program_b, spacing, price) = match orca::instructions::unpack(ix.data()).ok()? {
        orca::instructions::WhirlpoolInstruction::InitializePool(init) => {
            let a = orca::accounts::get_initialize_pool_accounts(ix).ok()?;
            (
                a.whirlpool,
                a.whirlpools_config,
                a.token_mint_a,
                a.token_mint_b,
                a.token_vault_a,
                a.token_vault_b,
                a.token_program,
                a.token_program,
                Some(init.tick_spacing),
                init.initial_sqrt_price,
            )
        }
        orca::instructions::WhirlpoolInstruction::InitializePoolV2(init) => {
            let a = orca::accounts::get_initialize_pool_v2_accounts(ix).ok()?;
            (
                a.whirlpool,
                a.whirlpools_config,
                a.token_mint_a,
                a.token_mint_b,
                a.token_vault_a,
                a.token_vault_b,
                a.token_program_a,
                a.token_program_b,
                Some(init.tick_spacing),
                init.initial_sqrt_price,
            )
        }
        orca::instructions::WhirlpoolInstruction::InitializePoolWithAdaptiveFee(init) => {
            let a = orca::accounts::get_initialize_pool_with_adaptive_fee_accounts(ix).ok()?;
            (
                a.whirlpool,
                a.whirlpools_config,
                a.token_mint_a,
                a.token_mint_b,
                a.token_vault_a,
                a.token_vault_b,
                a.token_program_a,
                a.token_program_b,
                None,
                init.initial_sqrt_price,
            )
        }
        _ => return None,
    };
    if event.whirlpool != pool
        || event.whirlpools_config != config
        || event.token_mint_a != mint_a
        || event.token_mint_b != mint_b
        || event.token_program_a != program_a
        || event.token_program_b != program_b
        || spacing.is_some_and(|spacing| spacing != event.tick_spacing)
        || price != event.initial_sqrt_price
    {
        return None;
    }
    Some(Candidate {
        protocol: Protocol::OrcaWhirlpool,
        address: pool.to_bytes(),
        program_id: orca::PROGRAM_ID,
        tokens: [
            Token {
                mint: mint_a.to_bytes(),
                vault: Some(vault_a.to_bytes()),
                decimals: Some(event.decimals_a),
                token_program: Some(program_a.to_bytes()),
            },
            Token {
                mint: mint_b.to_bytes(),
                vault: Some(vault_b.to_bytes()),
                decimals: Some(event.decimals_b),
                token_program: Some(program_b.to_bytes()),
            },
        ],
    })
}
