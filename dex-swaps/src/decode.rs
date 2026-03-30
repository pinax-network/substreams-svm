use common::solana::{get_fee_payer, get_signers, is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_data, parse_program_id, parse_raydium_log};
use proto::pb::{
    boop::v1 as boop_pb, darklake::v1 as darklake_pb, dex::swaps::v1 as pb, dumpfun::v1 as dumpfun_pb, jupiter::v1 as jupiter_pb,
    meteora::daam::v1 as meteora_daam_pb, meteora::dllm::v1 as meteora_dllm_pb, orca::v1 as orca_pb,
    pumpfun::amm::v1 as pumpfun_amm_pb, pumpfun::v1 as pumpfun_pb, raydium::amm::v1 as raydium_amm_pb, raydium::clmm::v1 as raydium_clmm_pb,
    raydium::cpmm::v1 as raydium_cpmm_pb, raydium::launchpad::v1 as raydium_launchpad_pb,
};
use substreams_solana::{
    block_view::InstructionView,
    pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta},
};
use substreams_solana_idls::{boop, darklake, dumpfun, jupiter, meteora, orca, pumpfun, raydium};

pub(crate) const PROTOCOL_BOOP: i32 = pb::Protocol::Boop as i32;
pub(crate) const PROTOCOL_DARKLAKE: i32 = pb::Protocol::Darklake as i32;
pub(crate) const PROTOCOL_DUMPFUN: i32 = pb::Protocol::Dumpfun as i32;
pub(crate) const PROTOCOL_JUPITER_V4: i32 = pb::Protocol::JupiterV4 as i32;
pub(crate) const PROTOCOL_JUPITER_V6: i32 = pb::Protocol::JupiterV6 as i32;
pub(crate) const PROTOCOL_METEORA_DAAM: i32 = pb::Protocol::MeteoraDaam as i32;
pub(crate) const PROTOCOL_METEORA_DLLM: i32 = pb::Protocol::MeteoraDllm as i32;
pub(crate) const PROTOCOL_ORCA_WHIRLPOOL: i32 = pb::Protocol::OrcaWhirlpool as i32;
pub(crate) const PROTOCOL_PUMPFUN: i32 = pb::Protocol::Pumpfun as i32;
pub(crate) const PROTOCOL_PUMPFUN_AMM: i32 = pb::Protocol::PumpfunAmm as i32;
pub(crate) const PROTOCOL_RAYDIUM_AMM_V4: i32 = pb::Protocol::RaydiumAmmV4 as i32;
pub(crate) const PROTOCOL_RAYDIUM_CLMM: i32 = pb::Protocol::RaydiumClmm as i32;
pub(crate) const PROTOCOL_RAYDIUM_CPMM: i32 = pb::Protocol::RaydiumCpmm as i32;
pub(crate) const PROTOCOL_RAYDIUM_LAUNCHPAD: i32 = pb::Protocol::RaydiumLaunchpad as i32;
pub(crate) fn pool_or_amm(amm: &[u8], amm_pool: &[u8]) -> Vec<u8> {
    if amm_pool.is_empty() {
        amm.to_vec()
    } else {
        amm_pool.to_vec()
    }
}

fn collect_program_logs<T, F>(tx_meta: &TransactionStatusMeta, program_id_bytes: &[u8], mut parser: F) -> Vec<T>
where
    F: FnMut(&str, u32) -> Option<T>,
{
    let mut out = Vec::new();
    let mut is_invoked = false;

    for log_message in tx_meta.log_messages.iter() {
        let matches_program = parse_program_id(log_message).map_or(false, |id| id == program_id_bytes);

        if is_invoke(log_message) && matches_program {
            if let Some(invoke_depth) = parse_invoke_depth(log_message) {
                is_invoked = true;
                if let Some(item) = parser(log_message, invoke_depth) {
                    out.push(item);
                }
            }
        } else if matches_program && (is_success(log_message) || is_failed(log_message)) {
            is_invoked = false;
        } else if is_invoked {
            if let Some(item) = parser(log_message, 0) {
                out.push(item);
            }
        }
    }

    out
}

pub(crate) fn decode_boop_transaction(tx: &ConfirmedTransaction) -> Option<boop_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(|ix| {
        let program_id = ix.program_id().0;
        if program_id != &boop::PROGRAM_ID {
            return None;
        }
        match boop::instructions::unpack(ix.data()) {
            Ok(boop::instructions::BoopInstruction::BuyToken(event)) => Some(boop_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(boop_pb::instruction::Instruction::Buy(boop_pb::BuyTokenInstruction {
                    buy_amount: event.buy_amount,
                    amount_out_min: event.amount_out_min,
                })),
            }),
            Ok(boop::instructions::BoopInstruction::SellToken(event)) => Some(boop_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(boop_pb::instruction::Instruction::Sell(boop_pb::SellTokenInstruction {
                    sell_amount: event.sell_amount,
                    amount_out_min: event.amount_out_min,
                })),
            }),
            _ => None,
        }
    }).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &boop::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_program_data(log_message)?;
        match boop::events::unpack_event(data.as_slice()) {
            Ok(boop::events::BoopEvent::TokenBoughtEvent(event)) => Some(boop_pb::Log {
                program_id: boop::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(boop_pb::log::Log::Bought(boop_pb::TokenBoughtEvent {
                    mint: event.mint.to_bytes().to_vec(),
                    amount_in: event.amount_in,
                    amount_out: event.amount_out,
                    swap_fee: event.swap_fee,
                    buyer: event.buyer.to_bytes().to_vec(),
                    recipient: event.recipient.to_bytes().to_vec(),
                })),
            }),
            Ok(boop::events::BoopEvent::TokenSoldEvent(event)) => Some(boop_pb::Log {
                program_id: boop::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(boop_pb::log::Log::Sold(boop_pb::TokenSoldEvent {
                    mint: event.mint.to_bytes().to_vec(),
                    amount_in: event.amount_in,
                    amount_out: event.amount_out,
                    swap_fee: event.swap_fee,
                    seller: event.seller.to_bytes().to_vec(),
                    recipient: event.recipient.to_bytes().to_vec(),
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(boop_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_darklake_transaction(tx: &ConfirmedTransaction) -> Option<darklake_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(|ix| {
        let program_id = ix.program_id().0;
        if program_id != &darklake::PROGRAM_ID {
            return None;
        }
        match darklake::instructions::unpack(ix.data()) {
            Ok(darklake::instructions::DarklakeInstruction::Swap(event)) => Some(darklake_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(darklake_pb::instruction::Instruction::Swap(darklake_pb::SwapInstruction {
                    amount_in: event.amount_in,
                    is_swap_x_to_y: event.is_swap_x_to_y,
                    c_min: event.c_min.to_vec(),
                    label: event.label.map(|l| l.to_vec()),
                })),
            }),
            _ => None,
        }
    }).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &darklake::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_program_data(log_message)?;
        match darklake::events::unpack_event(data.as_slice()) {
            Ok(darklake::events::DarklakeEvent::Swap(event)) => Some(darklake_pb::Log {
                program_id: darklake::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(darklake_pb::log::Log::Swap(darklake_pb::SwapEvent {
                    trader: event.trader.to_bytes().to_vec(),
                    direction: event.direction as u32,
                    deadline: event.deadline,
                    trade_fee: event.trade_fee,
                    protocol_fee: event.protocol_fee,
                    amount_in: event.amount_in,
                    amount_out: event.amount_out,
                    actual_amount_in: event.actual_amount_in,
                    wsol_deposit: event.wsol_deposit,
                    actual_amount_out: event.actual_amount_out,
                    new_reserve_x: event.new_reserve_x,
                    new_reserve_y: event.new_reserve_y,
                    available_reserve_x: event.available_reserve_x,
                    available_reserve_y: event.available_reserve_y,
                    locked_x: event.locked_x,
                    locked_y: event.locked_y,
                    user_locked_x: event.user_locked_x,
                    user_locked_y: event.user_locked_y,
                    protocol_fee_x: event.protocol_fee_x,
                    protocol_fee_y: event.protocol_fee_y,
                    user_token_account_x: event.user_token_account_x.to_bytes().to_vec(),
                    user_token_account_y: event.user_token_account_y.to_bytes().to_vec(),
                    token_mint_lp: event.token_mint_lp.to_bytes().to_vec(),
                    token_mint_x: event.token_mint_x.to_bytes().to_vec(),
                    token_mint_y: event.token_mint_y.to_bytes().to_vec(),
                    label: event.label,
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(darklake_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_dumpfun_transaction(tx: &ConfirmedTransaction) -> Option<dumpfun_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(|ix| {
        let program_id = ix.program_id().0;
        if program_id != &dumpfun::PROGRAM_ID { return None; }
        match dumpfun::instructions::unpack(ix.data()) {
            Ok(dumpfun::instructions::DumpfunInstruction::BuyExactTokens(event)) => Some(dumpfun_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(dumpfun_pb::instruction::Instruction::BuyExactTokens(dumpfun_pb::BuyExactTokensInstruction {
                    token_out: event.token_out,
                    max_sol_in: event.max_sol_in,
                })),
            }),
            Ok(dumpfun::instructions::DumpfunInstruction::BuyTokensWithExactSol(event)) => Some(dumpfun_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(dumpfun_pb::instruction::Instruction::BuyTokensWithExactSol(dumpfun_pb::BuyTokensWithExactSolInstruction {
                    sol_in: event.sol_in,
                    min_token_out: event.min_token_out,
                })),
            }),
            Ok(dumpfun::instructions::DumpfunInstruction::SellExactTokens(event)) => Some(dumpfun_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(dumpfun_pb::instruction::Instruction::SellExactTokens(dumpfun_pb::SellExactTokensInstruction {
                    token_in: event.token_in,
                    min_sol_out: event.min_sol_out,
                })),
            }),
            _ => None,
        }
    }).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &dumpfun::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_program_data(log_message)?;
        match dumpfun::events::unpack_event(data.as_slice()) {
            Ok(dumpfun::events::DumpfunEvent::BuyTokenEvent(event)) => Some(dumpfun_pb::Log {
                program_id: dumpfun::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(dumpfun_pb::log::Log::Buy(dumpfun_pb::BuyTokenEvent {
                    user: event.user.to_bytes().to_vec(),
                    mint: event.mint.to_bytes().to_vec(),
                    sol_in: event.sol_in,
                    token_out: event.token_out,
                    buy_time: event.buy_time,
                })),
            }),
            Ok(dumpfun::events::DumpfunEvent::SellTokenEvent(event)) => Some(dumpfun_pb::Log {
                program_id: dumpfun::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(dumpfun_pb::log::Log::Sell(dumpfun_pb::SellTokenEvent {
                    user: event.user.to_bytes().to_vec(),
                    mint: event.mint.to_bytes().to_vec(),
                    token_in: event.token_in,
                    sol_out: event.sol_out,
                    sell_time: event.sell_time,
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(dumpfun_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_jupiter_v4_transaction(tx: &ConfirmedTransaction) -> Option<jupiter_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let mut instructions = Vec::new();
    let mut is_invoked = false;
    let mut current_stack_height = 0;

    for log_message in &tx_meta.log_messages {
        let is_jupiter_program = parse_program_id(log_message).map_or(false, |id| id == jupiter::v4::PROGRAM_ID.to_vec());
        if is_invoke(log_message) && is_jupiter_program {
            if let Some(height) = parse_invoke_depth(log_message) {
                current_stack_height = height - 1;
                is_invoked = true;
                continue;
            }
        } else if is_jupiter_program && (is_success(log_message) || is_failed(log_message)) {
            is_invoked = false;
            continue;
        }
        if !is_invoked {
            continue;
        }
        let data = match parse_program_data(log_message) {
            Some(data) => data,
            None => continue,
        };
        if let Ok(jupiter::v4::events::JupiterV4Event::Swap(event)) = jupiter::v4::events::unpack(data.as_slice()) {
            instructions.push(jupiter_pb::Instruction {
                program_id: jupiter::v4::PROGRAM_ID.to_vec(),
                stack_height: current_stack_height,
                instruction: Some(jupiter_pb::instruction::Instruction::SwapEvent(jupiter_pb::SwapEvent {
                    amm: event.amm.to_bytes().to_vec(),
                    input_mint: event.input_mint.to_bytes().to_vec(),
                    input_amount: event.input_amount,
                    output_mint: event.output_mint.to_bytes().to_vec(),
                    output_amount: event.output_amount,
                })),
            });
        }
    }
    if instructions.is_empty() { return None; }
    Some(jupiter_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
    })
}

pub(crate) fn decode_jupiter_v6_transaction(tx: &ConfirmedTransaction) -> Option<jupiter_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(|instruction| {
        let program_id = instruction.program_id().0;
        if program_id != &jupiter::v6::PROGRAM_ID { return None; }
        match jupiter::v6::events::unpack(instruction.data()) {
            Ok(jupiter::v6::events::JupiterV6Event::Swap(event)) => Some(jupiter_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: instruction.stack_height(),
                instruction: Some(jupiter_pb::instruction::Instruction::SwapEvent(jupiter_pb::SwapEvent {
                    amm: event.amm.to_bytes().to_vec(),
                    input_mint: event.input_mint.to_bytes().to_vec(),
                    input_amount: event.input_amount,
                    output_mint: event.output_mint.to_bytes().to_vec(),
                    output_amount: event.output_amount,
                })),
            }),
            Ok(jupiter::v6::events::JupiterV6Event::Fee(event)) => Some(jupiter_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: instruction.stack_height(),
                instruction: Some(jupiter_pb::instruction::Instruction::FeeEvent(jupiter_pb::FeeEvent {
                    account: event.account.to_bytes().to_vec(),
                    mint: event.mint.to_bytes().to_vec(),
                    amount: event.amount,
                })),
            }),
            _ => None,
        }
    }).collect::<Vec<_>>();
    if instructions.is_empty() { return None; }
    Some(jupiter_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
    })
}

pub(crate) fn decode_meteora_daam_transaction(tx: &ConfirmedTransaction) -> Option<meteora_daam_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_meteora_daam_instruction).collect::<Vec<_>>();
    let logs = tx.walk_instructions().filter_map(decode_meteora_daam_event_instruction).collect::<Vec<_>>();
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(meteora_daam_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_meteora_dllm_transaction(tx: &ConfirmedTransaction) -> Option<meteora_dllm_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_meteora_dllm_instruction).collect::<Vec<_>>();
    if instructions.is_empty() { return None; }
    Some(meteora_dllm_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
    })
}

pub(crate) fn decode_orca_transaction(tx: &ConfirmedTransaction) -> Option<orca_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_orca_instruction).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &orca::whirlpool::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_program_data(log_message)?;
        match orca::whirlpool::events::parse_event(data.as_slice()) {
            Ok(orca::whirlpool::events::WhirlpoolEvent::Traded(event)) => Some(orca_pb::Log {
                program_id: orca::whirlpool::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(orca_pb::log::Log::Traded(orca_pb::TradedEvent {
                    whirlpool: event.whirlpool.to_bytes().to_vec(),
                    a_to_b: event.a_to_b,
                    pre_sqrt_price: event.pre_sqrt_price.to_string(),
                    post_sqrt_price: event.post_sqrt_price.to_string(),
                    input_amount: event.input_amount,
                    output_amount: event.output_amount,
                    input_transfer_fee: event.input_transfer_fee,
                    output_transfer_fee: event.output_transfer_fee,
                    lp_fee: event.lp_fee,
                    protocol_fee: event.protocol_fee,
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(orca_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_pumpfun_transaction(tx: &ConfirmedTransaction) -> Option<pumpfun_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_pumpfun_instruction).collect::<Vec<_>>();
    if instructions.is_empty() { return None; }
    Some(pumpfun_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
    })
}

pub(crate) fn decode_pumpfun_amm_transaction(tx: &ConfirmedTransaction) -> Option<pumpfun_amm_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_pumpfun_amm_instruction).collect::<Vec<_>>();
    if instructions.is_empty() { return None; }
    Some(pumpfun_amm_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
    })
}

pub(crate) fn decode_raydium_amm_v4_transaction(tx: &ConfirmedTransaction) -> Option<raydium_amm_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_raydium_amm_v4_instruction).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &raydium::amm::v4::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_raydium_log(log_message)?;
        match raydium::amm::v4::logs::unpack(data.as_slice()) {
            Ok(raydium::amm::v4::logs::RaydiumV4Log::SwapBaseIn(event)) => Some(raydium_amm_pb::Log {
                program_id: raydium::amm::v4::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(raydium_amm_pb::log::Log::SwapBaseIn(raydium_amm_pb::SwapBaseInLog {
                    amount_in: event.amount_in,
                    minimum_out: event.minimum_out,
                    direction: event.direction,
                    user_source: event.user_source,
                    pool_coin: event.pool_coin,
                    pool_pc: event.pool_pc,
                    out_amount: event.out_amount,
                })),
            }),
            Ok(raydium::amm::v4::logs::RaydiumV4Log::SwapBaseOut(event)) => Some(raydium_amm_pb::Log {
                program_id: raydium::amm::v4::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(raydium_amm_pb::log::Log::SwapBaseOut(raydium_amm_pb::SwapBaseOutLog {
                    max_in: event.max_in,
                    amount_out: event.amount_out,
                    direction: event.direction,
                    user_source: event.user_source,
                    pool_coin: event.pool_coin,
                    pool_pc: event.pool_pc,
                    deduct_in: event.deduct_in,
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(raydium_amm_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_raydium_clmm_transaction(tx: &ConfirmedTransaction) -> Option<raydium_clmm_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_raydium_clmm_instruction).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &raydium::clmm::v3::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_program_data(log_message)?;
        match raydium::clmm::v3::events::unpack(data.as_slice()) {
            Ok(raydium::clmm::v3::events::RaydiumClmmEvent::SwapEvent(event)) => Some(raydium_clmm_pb::Log {
                program_id: raydium::clmm::v3::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(raydium_clmm_pb::log::Log::Swap(raydium_clmm_pb::SwapLog {
                    pool_state: event.pool_state.to_bytes().to_vec(),
                    sender: event.sender.to_bytes().to_vec(),
                    token_account_0: event.token_account_0.to_bytes().to_vec(),
                    token_account_1: event.token_account_1.to_bytes().to_vec(),
                    amount_0: event.amount_0,
                    transfer_fee_0: event.transfer_fee_0,
                    amount_1: event.amount_1,
                    transfer_fee_1: event.transfer_fee_1,
                    zero_for_one: event.zero_for_one,
                    sqrt_price_x64: event.sqrt_price_x64.to_string(),
                    liquidity: event.liquidity.to_string(),
                    tick: event.tick,
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(raydium_clmm_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_raydium_cpmm_transaction(tx: &ConfirmedTransaction) -> Option<raydium_cpmm_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_raydium_cpmm_instruction).collect::<Vec<_>>();
    let logs = collect_program_logs(tx_meta, &raydium::cpmm::PROGRAM_ID.to_vec(), |log_message, invoke_depth| {
        let data = parse_program_data(log_message)?;
        match raydium::cpmm::events::unpack(data.as_slice()) {
            Ok(raydium::cpmm::events::RaydiumCpmmEvent::SwapEventV1(event)) => Some(raydium_cpmm_pb::Log {
                program_id: raydium::cpmm::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(raydium_cpmm_pb::log::Log::Swap(raydium_cpmm_pb::SwapEvent {
                    pool_id: event.pool_id.to_bytes().to_vec(),
                    input_vault_before: event.input_vault_before,
                    output_vault_before: event.output_vault_before,
                    input_amount: event.input_amount,
                    output_amount: event.output_amount,
                    input_transfer_fee: event.input_transfer_fee,
                    output_transfer_fee: event.output_transfer_fee,
                    base_input: event.base_input,
                    input_mint: None,
                    output_mint: None,
                    trade_fee: None,
                    creator_fee: None,
                    creator_fee_on_input: None,
                })),
            }),
            Ok(raydium::cpmm::events::RaydiumCpmmEvent::SwapEventV2(event)) => Some(raydium_cpmm_pb::Log {
                program_id: raydium::cpmm::PROGRAM_ID.to_vec(),
                invoke_depth,
                log: Some(raydium_cpmm_pb::log::Log::Swap(raydium_cpmm_pb::SwapEvent {
                    pool_id: event.pool_id.to_bytes().to_vec(),
                    input_vault_before: event.input_vault_before,
                    output_vault_before: event.output_vault_before,
                    input_amount: event.input_amount,
                    output_amount: event.output_amount,
                    input_transfer_fee: event.input_transfer_fee,
                    output_transfer_fee: event.output_transfer_fee,
                    base_input: event.base_input,
                    input_mint: Some(event.input_mint.to_bytes().to_vec()),
                    output_mint: Some(event.output_mint.to_bytes().to_vec()),
                    trade_fee: Some(event.trade_fee),
                    creator_fee: Some(event.creator_fee),
                    creator_fee_on_input: Some(event.creator_fee_on_input),
                })),
            }),
            _ => None,
        }
    });
    if instructions.is_empty() && logs.is_empty() { return None; }
    Some(raydium_cpmm_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
        logs,
    })
}

pub(crate) fn decode_raydium_launchpad_transaction(tx: &ConfirmedTransaction) -> Option<raydium_launchpad_pb::Transaction> {
    let tx_meta = tx.meta.as_ref()?;
    let instructions = tx.walk_instructions().filter_map(decode_raydium_launchpad_instruction).collect::<Vec<_>>();
    if instructions.is_empty() { return None; }
    Some(raydium_launchpad_pb::Transaction {
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(tx).unwrap_or_default(),
        signers: get_signers(tx).unwrap_or_default(),
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        instructions,
    })
}

pub(crate) fn decode_meteora_daam_instruction(ix: InstructionView) -> Option<meteora_daam_pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &meteora::daam::PROGRAM_ID { return None; }
    match meteora::daam::instructions::unpack(ix.data()) {
        Ok(meteora::daam::instructions::MeteoraDammInstruction::Swap(instr)) => {
            let accounts = meteora::daam::accounts::get_swap_accounts(&ix).ok()?;
            Some(meteora_daam_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(meteora_daam_pb::instruction::Instruction::Swap(meteora_daam_pb::SwapInstruction {
                    accounts: Some(meteora_daam_pb::SwapAccounts {
                        pool_authority: accounts.pool_authority.to_bytes().to_vec(),
                        pool: accounts.pool.to_bytes().to_vec(),
                        input_token_account: accounts.input_token_account.to_bytes().to_vec(),
                        output_token_account: accounts.output_token_account.to_bytes().to_vec(),
                        token_a_vault: accounts.token_a_vault.to_bytes().to_vec(),
                        token_b_vault: accounts.token_b_vault.to_bytes().to_vec(),
                        token_a_mint: accounts.token_a_mint.to_bytes().to_vec(),
                        token_b_mint: accounts.token_b_mint.to_bytes().to_vec(),
                        payer: accounts.payer.to_bytes().to_vec(),
                        token_a_program: accounts.token_a_program.to_bytes().to_vec(),
                        token_b_program: accounts.token_b_program.to_bytes().to_vec(),
                        referral_token_account: accounts.referral_token_account.map(|p| p.to_bytes().to_vec()),
                        event_authority: accounts.event_authority.to_bytes().to_vec(),
                        program: accounts.program.to_bytes().to_vec(),
                    }),
                    params: Some(meteora_daam_pb::SwapParameters {
                        amount_in: instr.params.amount_in,
                        minimum_amount_out: instr.params.minimum_amount_out,
                    }),
                })),
            })
        }
        _ => None,
    }
}

pub(crate) fn decode_meteora_daam_event_instruction(ix: InstructionView) -> Option<meteora_daam_pb::Log> {
    let program_id = ix.program_id().0;
    if program_id != &meteora::daam::PROGRAM_ID { return None; }
    match meteora::daam::anchor_cpi_event::unpack(ix.data()) {
        Ok(meteora::daam::anchor_cpi_event::MeteoraDammAnchorCpiEvent::EvtSwap(event)) => Some(meteora_daam_pb::Log {
            program_id: program_id.to_vec(),
            invoke_depth: ix.stack_height(),
            log: Some(meteora_daam_pb::log::Log::Swap(meteora_daam_pb::SwapLog {
                pool: event.pool.to_bytes().to_vec(),
                trade_direction: event.trade_direction as u32,
                has_referral: event.has_referral,
                params: Some(meteora_daam_pb::SwapParameters {
                    amount_in: event.params.amount_in,
                    minimum_amount_out: event.params.minimum_amount_out,
                }),
                result: Some(meteora_daam_pb::SwapResult {
                    output_amount: event.swap_result.output_amount,
                    next_sqrt_price: event.swap_result.next_sqrt_price.to_string(),
                    lp_fee: event.swap_result.lp_fee,
                    protocol_fee: event.swap_result.protocol_fee,
                    partner_fee: event.swap_result.partner_fee,
                    referral_fee: event.swap_result.referral_fee,
                }),
                actual_amount_in: event.actual_amount_in,
                current_timestamp: event.current_timestamp,
            })),
        }),
        _ => None,
    }
}

pub(crate) fn decode_meteora_dllm_instruction(ix: InstructionView) -> Option<meteora_dllm_pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &meteora::dllm::PROGRAM_ID { return None; }
    if let Ok(meteora::dllm::anchor_cpi_event::MeteoraDllmAnchorCpiEvent::Swap(event)) = meteora::dllm::anchor_cpi_event::unpack(ix.data()) {
        return Some(meteora_dllm_pb::Instruction {
            program_id: program_id.to_vec(),
            stack_height: ix.stack_height(),
            instruction: Some(meteora_dllm_pb::instruction::Instruction::SwapEvent(meteora_dllm_pb::SwapEvent {
                lb_pair: event.lb_pair.to_bytes().to_vec(),
                from: event.from.to_bytes().to_vec(),
                start_bin_id: event.start_bin_id,
                end_bin_id: event.end_bin_id,
                amount_in: event.amount_in,
                amount_out: event.amount_out,
                swap_for_y: event.swap_for_y,
                fee: event.fee,
                protocol_fee: event.protocol_fee,
                fee_bps: event.fee_bps.to_string(),
                host_fee: event.host_fee,
            })),
        });
    }
    match meteora::dllm::instructions::unpack(ix.data()) {
        Ok(meteora::dllm::instructions::MeteoraDllmInstruction::Swap(evt)) => {
            let accounts = meteora::dllm::accounts::get_swap_accounts(&ix).ok()?;
            Some(meteora_dllm_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(meteora_dllm_pb::instruction::Instruction::SwapInstruction(meteora_dllm_pb::SwapInstruction {
                    accounts: Some(meteora_dllm_pb::SwapAccounts {
                        lb_pair: accounts.lb_pair.to_bytes().to_vec(),
                        bin_array_bitmap_extension: accounts.bin_array_bitmap_extension.map(|a| a.to_bytes().to_vec()).unwrap_or_default(),
                        reserve_x: accounts.reserve_x.to_bytes().to_vec(),
                        reserve_y: accounts.reserve_y.to_bytes().to_vec(),
                        user_token_in: accounts.user_token_in.to_bytes().to_vec(),
                        user_token_out: accounts.user_token_out.to_bytes().to_vec(),
                        token_x_mint: accounts.token_x_mint.to_bytes().to_vec(),
                        token_y_mint: accounts.token_y_mint.to_bytes().to_vec(),
                        oracle: accounts.oracle.to_bytes().to_vec(),
                        host_fee_in: accounts.host_fee_in.map(|a| a.to_bytes().to_vec()).unwrap_or_default(),
                        user: accounts.user.to_bytes().to_vec(),
                        token_x_program: accounts.token_x_program.to_bytes().to_vec(),
                        token_y_program: accounts.token_y_program.to_bytes().to_vec(),
                        event_authority: accounts.event_authority.to_bytes().to_vec(),
                        program: accounts.program.to_bytes().to_vec(),
                    }),
                    amount_in: evt.amount_in,
                    min_amount_out: evt.min_amount_out,
                })),
            })
        }
        _ => None,
    }
}

pub(crate) fn decode_orca_instruction(ix: InstructionView) -> Option<orca_pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &orca::whirlpool::PROGRAM_ID { return None; }
    match orca::whirlpool::instructions::unpack(ix.data()) {
        Ok(orca::whirlpool::instructions::WhirlpoolInstruction::SwapV2(event)) => {
            let accounts = orca::whirlpool::accounts::get_swap_v2_accounts(&ix).ok()?;
            Some(orca_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(orca_pb::instruction::Instruction::SwapV2(orca_pb::SwapV2Instruction {
                    accounts: Some(orca_pb::SwapV2Accounts {
                        token_program_a: accounts.token_program_a.to_bytes().to_vec(),
                        token_program_b: accounts.token_program_b.to_bytes().to_vec(),
                        memo_program: accounts.memo_program.to_bytes().to_vec(),
                        token_authority: accounts.token_authority.to_bytes().to_vec(),
                        whirlpool: accounts.whirlpool.to_bytes().to_vec(),
                        token_mint_a: accounts.token_mint_a.to_bytes().to_vec(),
                        token_mint_b: accounts.token_mint_b.to_bytes().to_vec(),
                        token_owner_account_a: accounts.token_owner_account_a.to_bytes().to_vec(),
                        token_vault_a: accounts.token_vault_a.to_bytes().to_vec(),
                        token_owner_account_b: accounts.token_owner_account_b.to_bytes().to_vec(),
                        token_vault_b: accounts.token_vault_b.to_bytes().to_vec(),
                        tick_array0: accounts.tick_array0.to_bytes().to_vec(),
                        tick_array1: accounts.tick_array1.to_bytes().to_vec(),
                        tick_array2: accounts.tick_array2.to_bytes().to_vec(),
                        oracle: accounts.oracle.to_bytes().to_vec(),
                    }),
                    amount: event.amount,
                    other_amount_threshold: event.other_amount_threshold,
                    sqrt_price_limit: event.sqrt_price_limit.to_string(),
                    amount_specified_is_input: event.amount_specified_is_input,
                    a_to_b: event.a_to_b,
                })),
            })
        }
        _ => None,
    }
}

pub(crate) fn decode_pumpfun_instruction(instruction: InstructionView) -> Option<pumpfun_pb::Instruction> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpfun::bonding_curve::PROGRAM_ID { return None; }
    let parsed = match pumpfun::bonding_curve::instructions::unpack(instruction.data()) {
        Ok(pumpfun::bonding_curve::instructions::PumpFunInstruction::Buy(event)) => Some(pumpfun_pb::instruction::Instruction::Buy(pumpfun_pb::BuyInstruction {
            accounts: Some(pumpfun_pb::TradeAccounts {
                global: instruction.accounts()[0].0.to_vec(),
                fee_recipient: instruction.accounts()[1].0.to_vec(),
                mint: instruction.accounts()[2].0.to_vec(),
                bonding_curve: instruction.accounts()[3].0.to_vec(),
                associated_bonding_curve: instruction.accounts()[4].0.to_vec(),
                associated_user: instruction.accounts()[5].0.to_vec(),
                user: instruction.accounts()[6].0.to_vec(),
                creator_vault: instruction.accounts()[9].0.to_vec(),
            }),
            amount: event.amount,
            max_sol_cost: event.max_sol_cost,
        })),
        Ok(pumpfun::bonding_curve::instructions::PumpFunInstruction::Sell(event)) => Some(pumpfun_pb::instruction::Instruction::Sell(pumpfun_pb::SellInstruction {
            accounts: Some(pumpfun_pb::TradeAccounts {
                global: instruction.accounts()[0].0.to_vec(),
                fee_recipient: instruction.accounts()[1].0.to_vec(),
                mint: instruction.accounts()[2].0.to_vec(),
                bonding_curve: instruction.accounts()[3].0.to_vec(),
                associated_bonding_curve: instruction.accounts()[4].0.to_vec(),
                associated_user: instruction.accounts()[5].0.to_vec(),
                user: instruction.accounts()[6].0.to_vec(),
                creator_vault: instruction.accounts()[9].0.to_vec(),
            }),
            amount: event.amount,
            min_sol_output: event.min_sol_output,
        })),
        _ => match pumpfun::bonding_curve::events::unpack(instruction.data()) {
            Ok(pumpfun::bonding_curve::events::PumpFunEvent::TradeV3(event)) => Some(pumpfun_pb::instruction::Instruction::Trade(pumpfun_pb::TradeEvent {
                mint: event.mint.to_bytes().to_vec(),
                sol_amount: event.sol_amount,
                token_amount: event.token_amount,
                is_buy: event.is_buy,
                user: event.user.to_bytes().to_vec(),
                timestamp: event.timestamp,
                virtual_sol_reserves: event.virtual_sol_reserves,
                virtual_token_reserves: event.virtual_token_reserves,
                real_sol_reserves: Some(event.real_sol_reserves),
                real_token_reserves: Some(event.real_token_reserves),
                fee_recipient: Some(event.fee_recipient.to_bytes().to_vec()),
                fee_basis_points: Some(event.fee_basis_points),
                fee: Some(event.fee),
                creator: Some(event.creator.to_bytes().to_vec()),
                creator_fee_basis_points: Some(event.creator_fee_basis_points),
                creator_fee: Some(event.creator_fee),
            })),
            Ok(pumpfun::bonding_curve::events::PumpFunEvent::TradeV2(event)) => Some(pumpfun_pb::instruction::Instruction::Trade(pumpfun_pb::TradeEvent {
                mint: event.mint.to_bytes().to_vec(),
                sol_amount: event.sol_amount,
                token_amount: event.token_amount,
                is_buy: event.is_buy,
                user: event.user.to_bytes().to_vec(),
                timestamp: event.timestamp,
                virtual_sol_reserves: event.virtual_sol_reserves,
                virtual_token_reserves: event.virtual_token_reserves,
                real_sol_reserves: Some(event.real_sol_reserves),
                real_token_reserves: Some(event.real_token_reserves),
                fee_recipient: Some(event.fee_recipient.to_bytes().to_vec()),
                fee_basis_points: Some(event.fee_basis_points),
                fee: Some(event.fee),
                creator: Some(event.creator.to_bytes().to_vec()),
                creator_fee_basis_points: Some(event.creator_fee_basis_points),
                creator_fee: Some(event.creator_fee),
            })),
            _ => None,
        },
    }?;
    Some(pumpfun_pb::Instruction { program_id: program_id.to_vec(), stack_height: instruction.stack_height(), instruction: Some(parsed) })
}

pub(crate) fn decode_pumpfun_amm_instruction(instruction: InstructionView) -> Option<pumpfun_amm_pb::Instruction> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpfun::amm::PROGRAM_ID { return None; }
    let parsed = match pumpfun::amm::instructions::unpack(instruction.data()) {
        Ok(pumpfun::amm::instructions::PumpFunAmmInstruction::Buy(event)) => Some(pumpfun_amm_pb::instruction::Instruction::BuyInstruction(pumpfun_amm_pb::BuyInstruction {
            accounts: Some(pumpfun_amm_trade_accounts(&instruction)),
            base_amount_out: event.base_amount_out,
            max_quote_amount_in: event.max_quote_amount_in,
        })),
        Ok(pumpfun::amm::instructions::PumpFunAmmInstruction::Sell(event)) => Some(pumpfun_amm_pb::instruction::Instruction::SellInstruction(pumpfun_amm_pb::SellInstruction {
            accounts: Some(pumpfun_amm_trade_accounts(&instruction)),
            base_amount_in: event.base_amount_in,
            min_quote_amount_out: event.min_quote_amount_out,
        })),
        _ => match pumpfun::amm::events::unpack(instruction.data()) {
            Ok(pumpfun::amm::events::PumpFunAmmEvent::BuyEventV2(event)) => Some(pumpfun_amm_pb::instruction::Instruction::BuyEvent(pumpfun_amm_pb::BuyEvent {
                base_amount_out: event.base_amount_out,
                max_quote_amount_in: event.max_quote_amount_in,
                quote_amount_in: event.quote_amount_in,
                quote_amount_in_with_lp_fee: event.quote_amount_in_with_lp_fee,
                user_quote_amount_in: event.user_quote_amount_in,
                trade: Some(pumpfun_amm_trade_details_v2_buy(&event)),
            })),
            Ok(pumpfun::amm::events::PumpFunAmmEvent::SellEventV2(event)) => Some(pumpfun_amm_pb::instruction::Instruction::SellEvent(pumpfun_amm_pb::SellEvent {
                base_amount_in: event.base_amount_in,
                min_quote_amount_out: event.min_quote_amount_out,
                quote_amount_out: event.quote_amount_out,
                quote_amount_out_without_lp_fee: event.quote_amount_out_without_lp_fee,
                user_quote_amount_out: event.user_quote_amount_out,
                trade: Some(pumpfun_amm_trade_details_v2_sell(&event)),
            })),
            _ => None,
        },
    }?;
    Some(pumpfun_amm_pb::Instruction { program_id: program_id.to_vec(), stack_height: instruction.stack_height(), instruction: Some(parsed) })
}

fn pumpfun_amm_trade_accounts(instruction: &InstructionView) -> pumpfun_amm_pb::TradeAccounts {
    pumpfun_amm_pb::TradeAccounts {
        pool: instruction.accounts()[0].0.to_vec(),
        user: instruction.accounts()[1].0.to_vec(),
        global_config: instruction.accounts()[2].0.to_vec(),
        base_mint: instruction.accounts()[3].0.to_vec(),
        quote_mint: instruction.accounts()[4].0.to_vec(),
        user_base_token_account: instruction.accounts()[5].0.to_vec(),
        user_quote_token_account: instruction.accounts()[6].0.to_vec(),
        pool_base_token_account: instruction.accounts()[7].0.to_vec(),
        pool_quote_token_account: instruction.accounts()[8].0.to_vec(),
        protocol_fee_recipient: instruction.accounts()[9].0.to_vec(),
        protocol_fee_recipient_token_account: instruction.accounts()[10].0.to_vec(),
        coin_creator_vault_ata: instruction.accounts().get(17).map(|a| a.0.to_vec()),
        coin_creator_vault_authority: instruction.accounts().get(18).map(|a| a.0.to_vec()),
    }
}

fn pumpfun_amm_trade_details_v2_buy(event: &pumpfun::amm::events::BuyEventV2) -> pumpfun_amm_pb::TradeDetails {
    pumpfun_amm_pb::TradeDetails {
        user_base_token_reserves: event.user_base_token_reserves,
        user_quote_token_reserves: event.user_quote_token_reserves,
        pool_base_token_reserves: event.pool_base_token_reserves,
        pool_quote_token_reserves: event.pool_quote_token_reserves,
        lp_fee_basis_points: event.lp_fee_basis_points,
        lp_fee: event.lp_fee,
        protocol_fee_basis_points: event.protocol_fee_basis_points,
        protocol_fee: event.protocol_fee,
        pool: event.pool.to_bytes().to_vec(),
        user: event.user.to_bytes().to_vec(),
        user_base_token_account: event.user_base_token_account.to_bytes().to_vec(),
        user_quote_token_account: event.user_quote_token_account.to_bytes().to_vec(),
        protocol_fee_recipient: event.protocol_fee_recipient.to_bytes().to_vec(),
        protocol_fee_recipient_token_account: event.protocol_fee_recipient_token_account.to_bytes().to_vec(),
        coin_creator: Some(event.coin_creator.to_bytes().to_vec()),
        coin_creator_fee_basis_points: Some(event.coin_creator_fee_basis_points),
        coin_creator_fee: Some(event.coin_creator_fee),
    }
}

fn pumpfun_amm_trade_details_v2_sell(event: &pumpfun::amm::events::SellEventV2) -> pumpfun_amm_pb::TradeDetails {
    pumpfun_amm_pb::TradeDetails {
        user_base_token_reserves: event.user_base_token_reserves,
        user_quote_token_reserves: event.user_quote_token_reserves,
        pool_base_token_reserves: event.pool_base_token_reserves,
        pool_quote_token_reserves: event.pool_quote_token_reserves,
        lp_fee_basis_points: event.lp_fee_basis_points,
        lp_fee: event.lp_fee,
        protocol_fee_basis_points: event.protocol_fee_basis_points,
        protocol_fee: event.protocol_fee,
        pool: event.pool.to_bytes().to_vec(),
        user: event.user.to_bytes().to_vec(),
        user_base_token_account: event.user_base_token_account.to_bytes().to_vec(),
        user_quote_token_account: event.user_quote_token_account.to_bytes().to_vec(),
        protocol_fee_recipient: event.protocol_fee_recipient.to_bytes().to_vec(),
        protocol_fee_recipient_token_account: event.protocol_fee_recipient_token_account.to_bytes().to_vec(),
        coin_creator: Some(event.coin_creator.to_bytes().to_vec()),
        coin_creator_fee_basis_points: Some(event.coin_creator_fee_basis_points),
        coin_creator_fee: Some(event.coin_creator_fee),
    }
}

pub(crate) fn decode_raydium_amm_v4_instruction(instruction: InstructionView) -> Option<raydium_amm_pb::Instruction> {
    let program_id = instruction.program_id().0;
    if program_id != &raydium::amm::v4::PROGRAM_ID { return None; }
    match raydium::amm::v4::instructions::unpack(instruction.data()) {
        Ok(raydium::amm::v4::instructions::RaydiumV4Instruction::SwapBaseIn(event)) => Some(raydium_amm_pb::Instruction {
            program_id: program_id.to_vec(),
            stack_height: instruction.stack_height(),
            instruction: Some(raydium_amm_pb::instruction::Instruction::SwapBaseIn(raydium_amm_pb::SwapBaseInInstruction {
                accounts: Some(raydium_amm_v4_swap_accounts(&instruction)),
                amount_in: event.amount_in,
                minimum_amount_out: event.minimum_amount_out,
            })),
        }),
        Ok(raydium::amm::v4::instructions::RaydiumV4Instruction::SwapBaseOut(event)) => Some(raydium_amm_pb::Instruction {
            program_id: program_id.to_vec(),
            stack_height: instruction.stack_height(),
            instruction: Some(raydium_amm_pb::instruction::Instruction::SwapBaseOut(raydium_amm_pb::SwapBaseOutInstruction {
                accounts: Some(raydium_amm_v4_swap_accounts(&instruction)),
                amount_out: event.amount_out,
                max_amount_in: event.max_amount_in,
            })),
        }),
        _ => None,
    }
}

fn raydium_amm_v4_swap_accounts(ix: &InstructionView) -> raydium_amm_pb::SwapAccounts {
    let with_target_orders = ix.accounts().len() == 18;
    let offset = if with_target_orders { 1 } else { 0 };
    raydium_amm_pb::SwapAccounts {
        token_program: ix.accounts()[0].0.to_vec(),
        amm: ix.accounts()[1].0.to_vec(),
        amm_authority: ix.accounts()[2].0.to_vec(),
        amm_open_orders: ix.accounts()[3].0.to_vec(),
        amm_target_orders: if with_target_orders { Some(ix.accounts()[4].0.to_vec()) } else { None },
        amm_coin_vault: ix.accounts()[4 + offset].0.to_vec(),
        amm_pc_vault: ix.accounts()[5 + offset].0.to_vec(),
        market_program: ix.accounts()[6 + offset].0.to_vec(),
        market: ix.accounts()[7 + offset].0.to_vec(),
        market_bids: ix.accounts()[8 + offset].0.to_vec(),
        market_asks: ix.accounts()[9 + offset].0.to_vec(),
        market_event_queue: ix.accounts()[10 + offset].0.to_vec(),
        market_coin_vault: ix.accounts()[11 + offset].0.to_vec(),
        market_pc_vault: ix.accounts()[12 + offset].0.to_vec(),
        market_vault_signer: ix.accounts()[13 + offset].0.to_vec(),
        user_token_source: ix.accounts()[14 + offset].0.to_vec(),
        user_token_destination: ix.accounts()[15 + offset].0.to_vec(),
        user_source_owner: ix.accounts()[16 + offset].0.to_vec(),
    }
}

pub(crate) fn decode_raydium_clmm_instruction(ix: InstructionView) -> Option<raydium_clmm_pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::clmm::v3::PROGRAM_ID { return None; }
    match raydium::clmm::v3::instructions::unpack(ix.data()) {
        Ok(raydium::clmm::v3::instructions::RaydiumClmmInstruction::SwapV2(event)) => {
            let accounts = raydium::clmm::v3::accounts::get_swap_v2_accounts(&ix).ok()?;
            Some(raydium_clmm_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_clmm_pb::instruction::Instruction::Swap(raydium_clmm_pb::SwapInstruction {
                    accounts: Some(raydium_clmm_pb::swap_instruction::Accounts::V2Accounts(raydium_clmm_pb::SwapV2Accounts {
                        payer: accounts.payer.to_bytes().to_vec(),
                        amm_config: accounts.amm_config.to_bytes().to_vec(),
                        pool_state: accounts.pool_state.to_bytes().to_vec(),
                        input_token_account: accounts.input_token_account.to_bytes().to_vec(),
                        output_token_account: accounts.output_token_account.to_bytes().to_vec(),
                        input_vault: accounts.input_vault.to_bytes().to_vec(),
                        output_vault: accounts.output_vault.to_bytes().to_vec(),
                        observation_state: accounts.observation_state.to_bytes().to_vec(),
                        token_program: accounts.token_program.to_bytes().to_vec(),
                        token_program_2022: accounts.token_program_2022.to_bytes().to_vec(),
                        memo_program: accounts.memo_program.to_bytes().to_vec(),
                        input_vault_mint: accounts.input_vault_mint.to_bytes().to_vec(),
                        output_vault_mint: accounts.output_vault_mint.to_bytes().to_vec(),
                    })),
                    amount: event.amount,
                    other_amount_threshold: event.other_amount_threshold,
                    sqrt_price_limit_x64: event.sqrt_price_limit_x64.to_string(),
                    is_base_input: event.is_base_input,
                })),
            })
        }
        _ => None,
    }
}

pub(crate) fn decode_raydium_cpmm_instruction(ix: InstructionView) -> Option<raydium_cpmm_pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::cpmm::PROGRAM_ID { return None; }
    match raydium::cpmm::instructions::unpack(ix.data()) {
        Ok(raydium::cpmm::instructions::RaydiumCpmmInstruction::SwapBaseInput(event)) => {
            let accounts = raydium::cpmm::accounts::get_swap_base_input_accounts(&ix).ok()?;
            Some(raydium_cpmm_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_cpmm_pb::instruction::Instruction::SwapBaseInput(raydium_cpmm_pb::SwapBaseInputInstruction {
                    accounts: Some(cpmm_swap_accounts_input(&accounts)),
                    amount_in: event.amount_in,
                    minimum_amount_out: event.minimum_amount_out,
                })),
            })
        }
        Ok(raydium::cpmm::instructions::RaydiumCpmmInstruction::SwapBaseOutput(event)) => {
            let accounts = raydium::cpmm::accounts::get_swap_base_output_accounts(&ix).ok()?;
            Some(raydium_cpmm_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_cpmm_pb::instruction::Instruction::SwapBaseOutput(raydium_cpmm_pb::SwapBaseOutputInstruction {
                    accounts: Some(cpmm_swap_accounts_output(&accounts)),
                    max_amount_in: event.max_amount_in,
                    amount_out: event.amount_out,
                })),
            })
        }
        _ => None,
    }
}

pub(crate) fn decode_raydium_launchpad_instruction(ix: InstructionView) -> Option<raydium_launchpad_pb::Instruction> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::launchpad::PROGRAM_ID { return None; }
    if let Ok(event) = raydium::launchpad::anchor_cpi_event::unpack(ix.data()) {
        return match event {
            raydium::launchpad::anchor_cpi_event::RaydiumLaunchpadAnchorCpiEvent::TradeEventV1(event) => Some(raydium_launchpad_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_launchpad_pb::instruction::Instruction::TradeEvent(raydium_launchpad_pb::TradeEvent {
                    pool_state: event.pool_state.to_bytes().to_vec(),
                    total_base_sell: event.total_base_sell,
                    virtual_base: event.virtual_base,
                    virtual_quote: event.virtual_quote,
                    real_base_before: event.real_base_before,
                    real_quote_before: event.real_quote_before,
                    real_base_after: event.real_base_after,
                    real_quote_after: event.real_quote_after,
                    amount_in: event.amount_in,
                    amount_out: event.amount_out,
                    protocol_fee: event.protocol_fee,
                    platform_fee: event.platform_fee,
                    creator_fee: Some(event.creator_fee),
                    share_fee: event.share_fee,
                    trade_direction: match event.trade_direction {
                        raydium::launchpad::anchor_cpi_event::TradeDirection::Buy => raydium_launchpad_pb::TradeDirection::Buy as i32,
                        raydium::launchpad::anchor_cpi_event::TradeDirection::Sell => raydium_launchpad_pb::TradeDirection::Sell as i32,
                    },
                    pool_status: match event.pool_status {
                        raydium::launchpad::anchor_cpi_event::PoolStatus::Fund => raydium_launchpad_pb::PoolStatus::Fund as i32,
                        raydium::launchpad::anchor_cpi_event::PoolStatus::Migrate => raydium_launchpad_pb::PoolStatus::Migrate as i32,
                        raydium::launchpad::anchor_cpi_event::PoolStatus::Trade => raydium_launchpad_pb::PoolStatus::Trade as i32,
                    },
                    exact_in: Some(event.exact_in),
                })),
            }),
            raydium::launchpad::anchor_cpi_event::RaydiumLaunchpadAnchorCpiEvent::TradeEventV2(event) => Some(raydium_launchpad_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_launchpad_pb::instruction::Instruction::TradeEvent(raydium_launchpad_pb::TradeEvent {
                    pool_state: event.pool_state.to_bytes().to_vec(),
                    total_base_sell: event.total_base_sell,
                    virtual_base: event.virtual_base,
                    virtual_quote: event.virtual_quote,
                    real_base_before: event.real_base_before,
                    real_quote_before: event.real_quote_before,
                    real_base_after: event.real_base_after,
                    real_quote_after: event.real_quote_after,
                    amount_in: event.amount_in,
                    amount_out: event.amount_out,
                    protocol_fee: event.protocol_fee,
                    platform_fee: event.platform_fee,
                    creator_fee: None,
                    share_fee: event.share_fee,
                    trade_direction: match event.trade_direction {
                        raydium::launchpad::anchor_cpi_event::TradeDirection::Buy => raydium_launchpad_pb::TradeDirection::Buy as i32,
                        raydium::launchpad::anchor_cpi_event::TradeDirection::Sell => raydium_launchpad_pb::TradeDirection::Sell as i32,
                    },
                    pool_status: match event.pool_status {
                        raydium::launchpad::anchor_cpi_event::PoolStatus::Fund => raydium_launchpad_pb::PoolStatus::Fund as i32,
                        raydium::launchpad::anchor_cpi_event::PoolStatus::Migrate => raydium_launchpad_pb::PoolStatus::Migrate as i32,
                        raydium::launchpad::anchor_cpi_event::PoolStatus::Trade => raydium_launchpad_pb::PoolStatus::Trade as i32,
                    },
                    exact_in: None,
                })),
            }),
            _ => None,
        };
    }
    match raydium::launchpad::instructions::unpack(ix.data()) {
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::BuyExactIn(evt)) => {
            let accounts = raydium::launchpad::accounts::get_buy_exact_in_accounts(&ix).ok()?;
            Some(raydium_launchpad_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_launchpad_pb::instruction::Instruction::BuyExactIn(raydium_launchpad_pb::BuyExactInInstruction {
                    accounts: Some(launchpad_trade_accounts(&accounts)),
                    amount_in: evt.amount_in,
                    minimum_amount_out: evt.minimum_amount_out,
                    share_fee_rate: evt.share_fee_rate,
                })),
            })
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::BuyExactOut(evt)) => {
            let accounts = raydium::launchpad::accounts::get_buy_exact_out_accounts(&ix).ok()?;
            Some(raydium_launchpad_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_launchpad_pb::instruction::Instruction::BuyExactOut(raydium_launchpad_pb::BuyExactOutInstruction {
                    accounts: Some(launchpad_trade_accounts(&accounts)),
                    amount_out: evt.amount_out,
                    maximum_amount_in: evt.maximum_amount_in,
                    share_fee_rate: evt.share_fee_rate,
                })),
            })
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::SellExactIn(evt)) => {
            let accounts = raydium::launchpad::accounts::get_sell_exact_in_accounts(&ix).ok()?;
            Some(raydium_launchpad_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_launchpad_pb::instruction::Instruction::SellExactIn(raydium_launchpad_pb::SellExactInInstruction {
                    accounts: Some(launchpad_trade_accounts(&accounts)),
                    amount_in: evt.amount_in,
                    minimum_amount_out: evt.minimum_amount_out,
                    share_fee_rate: evt.share_fee_rate,
                })),
            })
        }
        Ok(raydium::launchpad::instructions::RaydiumLaunchpadInstruction::SellExactOut(evt)) => {
            let accounts = raydium::launchpad::accounts::get_sell_exact_out_accounts(&ix).ok()?;
            Some(raydium_launchpad_pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: ix.stack_height(),
                instruction: Some(raydium_launchpad_pb::instruction::Instruction::SellExactOut(raydium_launchpad_pb::SellExactOutInstruction {
                    accounts: Some(launchpad_trade_accounts(&accounts)),
                    amount_out: evt.amount_out,
                    maximum_amount_in: evt.maximum_amount_in,
                    share_fee_rate: evt.share_fee_rate,
                })),
            })
        }
        _ => None,
    }
}

fn cpmm_swap_accounts_input(accounts: &raydium::cpmm::accounts::SwapBaseInputAccounts) -> raydium_cpmm_pb::SwapAccounts {
    raydium_cpmm_pb::SwapAccounts {
        payer: accounts.payer.to_bytes().to_vec(),
        authority: accounts.authority.to_bytes().to_vec(),
        amm_config: accounts.amm_config.to_bytes().to_vec(),
        pool_state: accounts.pool_state.to_bytes().to_vec(),
        input_token_account: accounts.input_token_account.to_bytes().to_vec(),
        output_token_account: accounts.output_token_account.to_bytes().to_vec(),
        input_vault: accounts.input_vault.to_bytes().to_vec(),
        output_vault: accounts.output_vault.to_bytes().to_vec(),
        input_token_program: accounts.input_token_program.to_bytes().to_vec(),
        output_token_program: accounts.output_token_program.to_bytes().to_vec(),
        input_token_mint: accounts.input_token_mint.to_bytes().to_vec(),
        output_token_mint: accounts.output_token_mint.to_bytes().to_vec(),
        observation_state: accounts.observation_state.to_bytes().to_vec(),
    }
}

fn cpmm_swap_accounts_output(accounts: &raydium::cpmm::accounts::SwapBaseOutputAccounts) -> raydium_cpmm_pb::SwapAccounts {
    raydium_cpmm_pb::SwapAccounts {
        payer: accounts.payer.to_bytes().to_vec(),
        authority: accounts.authority.to_bytes().to_vec(),
        amm_config: accounts.amm_config.to_bytes().to_vec(),
        pool_state: accounts.pool_state.to_bytes().to_vec(),
        input_token_account: accounts.input_token_account.to_bytes().to_vec(),
        output_token_account: accounts.output_token_account.to_bytes().to_vec(),
        input_vault: accounts.input_vault.to_bytes().to_vec(),
        output_vault: accounts.output_vault.to_bytes().to_vec(),
        input_token_program: accounts.input_token_program.to_bytes().to_vec(),
        output_token_program: accounts.output_token_program.to_bytes().to_vec(),
        input_token_mint: accounts.input_token_mint.to_bytes().to_vec(),
        output_token_mint: accounts.output_token_mint.to_bytes().to_vec(),
        observation_state: accounts.observation_state.to_bytes().to_vec(),
    }
}

fn launchpad_trade_accounts<T>(accounts: &T) -> raydium_launchpad_pb::TradeAccounts
where
    T: LaunchpadTradeAccountsLike,
{
    raydium_launchpad_pb::TradeAccounts {
        payer: accounts.payer(),
        authority: accounts.authority(),
        global_config: accounts.global_config(),
        platform_config: accounts.platform_config(),
        pool_state: accounts.pool_state(),
        user_base_token: accounts.user_base_token(),
        user_quote_token: accounts.user_quote_token(),
        base_vault: accounts.base_vault(),
        quote_vault: accounts.quote_vault(),
        base_token_mint: accounts.base_token_mint(),
        quote_token_mint: accounts.quote_token_mint(),
        base_token_program: accounts.base_token_program(),
        quote_token_program: accounts.quote_token_program(),
        event_authority: accounts.event_authority(),
        program: accounts.program(),
    }
}

trait LaunchpadTradeAccountsLike {
    fn payer(&self) -> Vec<u8>;
    fn authority(&self) -> Vec<u8>;
    fn global_config(&self) -> Vec<u8>;
    fn platform_config(&self) -> Vec<u8>;
    fn pool_state(&self) -> Vec<u8>;
    fn user_base_token(&self) -> Vec<u8>;
    fn user_quote_token(&self) -> Vec<u8>;
    fn base_vault(&self) -> Vec<u8>;
    fn quote_vault(&self) -> Vec<u8>;
    fn base_token_mint(&self) -> Vec<u8>;
    fn quote_token_mint(&self) -> Vec<u8>;
    fn base_token_program(&self) -> Vec<u8>;
    fn quote_token_program(&self) -> Vec<u8>;
    fn event_authority(&self) -> Vec<u8>;
    fn program(&self) -> Vec<u8>;
}

macro_rules! impl_launchpad_trade_accounts_like {
    ($ty:path) => {
        impl LaunchpadTradeAccountsLike for $ty {
            fn payer(&self) -> Vec<u8> { self.payer.to_bytes().to_vec() }
            fn authority(&self) -> Vec<u8> { self.authority.to_bytes().to_vec() }
            fn global_config(&self) -> Vec<u8> { self.global_config.to_bytes().to_vec() }
            fn platform_config(&self) -> Vec<u8> { self.platform_config.to_bytes().to_vec() }
            fn pool_state(&self) -> Vec<u8> { self.pool_state.to_bytes().to_vec() }
            fn user_base_token(&self) -> Vec<u8> { self.user_base_token.to_bytes().to_vec() }
            fn user_quote_token(&self) -> Vec<u8> { self.user_quote_token.to_bytes().to_vec() }
            fn base_vault(&self) -> Vec<u8> { self.base_vault.to_bytes().to_vec() }
            fn quote_vault(&self) -> Vec<u8> { self.quote_vault.to_bytes().to_vec() }
            fn base_token_mint(&self) -> Vec<u8> { self.base_token_mint.to_bytes().to_vec() }
            fn quote_token_mint(&self) -> Vec<u8> { self.quote_token_mint.to_bytes().to_vec() }
            fn base_token_program(&self) -> Vec<u8> { self.base_token_program.to_bytes().to_vec() }
            fn quote_token_program(&self) -> Vec<u8> { self.quote_token_program.to_bytes().to_vec() }
            fn event_authority(&self) -> Vec<u8> { self.event_authority.to_bytes().to_vec() }
            fn program(&self) -> Vec<u8> { self.program.to_bytes().to_vec() }
        }
    };
}

impl_launchpad_trade_accounts_like!(raydium::launchpad::accounts::BuyExactInAccounts);
impl_launchpad_trade_accounts_like!(raydium::launchpad::accounts::BuyExactOutAccounts);
impl_launchpad_trade_accounts_like!(raydium::launchpad::accounts::SellExactInAccounts);
impl_launchpad_trade_accounts_like!(raydium::launchpad::accounts::SellExactOutAccounts);
