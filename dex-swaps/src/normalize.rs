use crate::decode::{
    pool_or_amm, PROTOCOL_BOOP, PROTOCOL_DARKLAKE, PROTOCOL_DUMPFUN,
    PROTOCOL_METEORA_DAAM, PROTOCOL_METEORA_DLLM, PROTOCOL_ORCA_WHIRLPOOL, PROTOCOL_PUMPFUN, PROTOCOL_PUMPFUN_AMM,
    PROTOCOL_RAYDIUM_AMM_V4, PROTOCOL_RAYDIUM_CLMM, PROTOCOL_RAYDIUM_CPMM, PROTOCOL_RAYDIUM_LAUNCHPAD,
};
use proto::pb::{
    boop::v1 as boop_pb, darklake::v1 as darklake_pb, dex::swaps::v1 as pb, dumpfun::v1 as dumpfun_pb, jupiter::v1 as jupiter_pb,
    meteora::daam::v1 as meteora_daam_pb, meteora::dllm::v1 as meteora_dllm_pb, orca::v1 as orca_pb,
    pumpfun::amm::v1 as pumpfun_amm_pb, pumpfun::v1 as pumpfun_pb, raydium::amm::v1 as raydium_amm_pb, raydium::clmm::v1 as raydium_clmm_pb,
    raydium::cpmm::v1 as raydium_cpmm_pb, raydium::launchpad::v1 as raydium_launchpad_pb,
};

const SOL_MINT: &[u8] = b"So11111111111111111111111111111111111111111";
const RAYDIUM_PC2COIN: u64 = 1;
pub(crate) fn map_boop_transaction(transaction: boop_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .logs
        .into_iter()
        .filter_map(|log| match log.log {
            Some(boop_pb::log::Log::Bought(event)) => Some(pb::Swap {
                protocol: PROTOCOL_BOOP,
                amm: log.program_id.clone(),
                amm_pool: log.program_id,
                user: event.buyer,
                input_mint: SOL_MINT.to_vec(),
                input_amount: event.amount_in,
                output_mint: event.mint,
                output_amount: event.amount_out,
            }),
            Some(boop_pb::log::Log::Sold(event)) => Some(pb::Swap {
                protocol: PROTOCOL_BOOP,
                amm: log.program_id.clone(),
                amm_pool: log.program_id,
                user: event.seller,
                input_mint: event.mint,
                input_amount: event.amount_in,
                output_mint: SOL_MINT.to_vec(),
                output_amount: event.amount_out,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_darklake_transaction(transaction: darklake_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, _)| map_darklake_swap(&transaction, instruction_index))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_darklake_swap(transaction: &darklake_pb::Transaction, instruction_index: usize) -> Option<pb::Swap> {
    let instruction = transaction.instructions.get(instruction_index)?;

    match instruction.instruction.as_ref()? {
        darklake_pb::instruction::Instruction::Swap(_) => {}
    }

    let event = transaction.logs[instruction_index..].iter().find_map(|log| match &log.log {
        Some(darklake_pb::log::Log::Swap(event)) => Some(event),
        _ => None,
    })?;

    let (input_mint, output_mint) = if event.direction == 0 {
        (&event.token_mint_x, &event.token_mint_y)
    } else {
        (&event.token_mint_y, &event.token_mint_x)
    };

    Some(pb::Swap {
        protocol: PROTOCOL_DARKLAKE,
        amm: instruction.program_id.clone(),
        amm_pool: instruction.program_id.clone(),
        user: event.trader.clone(),
        input_mint: input_mint.clone(),
        input_amount: event.amount_in,
        output_mint: output_mint.clone(),
        output_amount: event.amount_out,
    })
}

pub(crate) fn map_dumpfun_transaction(transaction: dumpfun_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .logs
        .into_iter()
        .filter_map(|log| match log.log {
            Some(dumpfun_pb::log::Log::Buy(event)) => Some(pb::Swap {
                protocol: PROTOCOL_DUMPFUN,
                amm: log.program_id.clone(),
                amm_pool: log.program_id,
                user: event.user,
                input_mint: SOL_MINT.to_vec(),
                input_amount: event.sol_in,
                output_mint: event.mint,
                output_amount: event.token_out,
            }),
            Some(dumpfun_pb::log::Log::Sell(event)) => Some(pb::Swap {
                protocol: PROTOCOL_DUMPFUN,
                amm: log.program_id.clone(),
                amm_pool: log.program_id,
                user: event.user,
                input_mint: event.mint,
                input_amount: event.token_in,
                output_mint: SOL_MINT.to_vec(),
                output_amount: event.sol_out,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_jupiter_transaction(transaction: jupiter_pb::Transaction, protocol: i32) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .into_iter()
        .filter_map(|instruction| match instruction.instruction {
            Some(jupiter_pb::instruction::Instruction::SwapEvent(event)) => Some(pb::Swap {
                protocol,
                amm: instruction.program_id.clone(),
                amm_pool: pool_or_amm(&instruction.program_id, &event.amm),
                user: transaction.fee_payer.clone(),
                input_mint: event.input_mint,
                input_amount: event.input_amount,
                output_mint: event.output_mint,
                output_amount: event.output_amount,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_orca_transaction(transaction: orca_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| map_orca_swap(&transaction, instruction, instruction_index))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_orca_swap(transaction: &orca_pb::Transaction, instruction: &orca_pb::Instruction, instruction_index: usize) -> Option<pb::Swap> {
    let traded_event = transaction.logs[instruction_index..].iter().find_map(|log| match &log.log {
        Some(orca_pb::log::Log::Traded(event)) => Some(event),
        _ => None,
    })?;

    match &instruction.instruction {
        Some(orca_pb::instruction::Instruction::SwapV2(event)) => {
            let accounts = event.accounts.as_ref()?;
            let (input_mint, output_mint) = if event.a_to_b {
                (&accounts.token_mint_a, &accounts.token_mint_b)
            } else {
                (&accounts.token_mint_b, &accounts.token_mint_a)
            };

            Some(pb::Swap {
                protocol: PROTOCOL_ORCA_WHIRLPOOL,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.whirlpool.clone(),
                user: accounts.token_authority.clone(),
                input_mint: input_mint.clone(),
                input_amount: traded_event.input_amount,
                output_mint: output_mint.clone(),
                output_amount: traded_event.output_amount,
            })
        }
        _ => None,
    }
}

pub(crate) fn map_raydium_amm_v4_transaction(transaction: raydium_amm_pb::Transaction) -> Option<pb::Transaction> {
    if transaction.logs.len() != transaction.instructions.len() {
        return None;
    }

    let swaps = transaction
        .instructions
        .iter()
        .zip(transaction.logs.iter())
        .filter_map(|(instruction, log)| map_raydium_amm_v4_swap(instruction, log))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_raydium_amm_v4_swap(instruction: &raydium_amm_pb::Instruction, log: &raydium_amm_pb::Log) -> Option<pb::Swap> {
    match (&instruction.instruction, &log.log) {
        (Some(raydium_amm_pb::instruction::Instruction::SwapBaseIn(event)), Some(raydium_amm_pb::log::Log::SwapBaseIn(traded_event))) => {
            let accounts = event.accounts.as_ref()?;
            let pc_to_coin = traded_event.direction == RAYDIUM_PC2COIN;
            let (input_mint, output_mint) = if pc_to_coin {
                (&accounts.amm_pc_vault, &accounts.amm_coin_vault)
            } else {
                (&accounts.amm_coin_vault, &accounts.amm_pc_vault)
            };

            Some(pb::Swap {
                protocol: PROTOCOL_RAYDIUM_AMM_V4,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.amm.clone(),
                user: accounts.user_source_owner.clone(),
                input_mint: input_mint.clone(),
                input_amount: event.amount_in,
                output_mint: output_mint.clone(),
                output_amount: traded_event.out_amount,
            })
        }
        (Some(raydium_amm_pb::instruction::Instruction::SwapBaseOut(event)), Some(raydium_amm_pb::log::Log::SwapBaseOut(traded_event))) => {
            let accounts = event.accounts.as_ref()?;
            let pc_to_coin = traded_event.direction == RAYDIUM_PC2COIN;
            let (input_mint, output_mint) = if pc_to_coin {
                (&accounts.amm_pc_vault, &accounts.amm_coin_vault)
            } else {
                (&accounts.amm_coin_vault, &accounts.amm_pc_vault)
            };

            Some(pb::Swap {
                protocol: PROTOCOL_RAYDIUM_AMM_V4,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.amm.clone(),
                user: accounts.user_source_owner.clone(),
                input_mint: input_mint.clone(),
                input_amount: traded_event.deduct_in,
                output_mint: output_mint.clone(),
                output_amount: event.amount_out,
            })
        }
        _ => None,
    }
}

pub(crate) fn map_pumpfun_transaction(transaction: pumpfun_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| map_pumpfun_swap(&transaction, instruction, instruction_index))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_pumpfun_swap(transaction: &pumpfun_pb::Transaction, instruction: &pumpfun_pb::Instruction, instruction_index: usize) -> Option<pb::Swap> {
    let trade_event = match transaction.instructions.get(instruction_index + 1)?.instruction.as_ref()? {
        pumpfun_pb::instruction::Instruction::Trade(event) => event,
        _ => return None,
    };

    match &instruction.instruction {
        Some(pumpfun_pb::instruction::Instruction::Buy(event)) => {
            let accounts = event.accounts.as_ref()?;
            let protocol_fee = trade_event.fee.unwrap_or(0);
            let creator_fee = trade_event.creator_fee.unwrap_or(0);
            Some(pb::Swap {
                protocol: PROTOCOL_PUMPFUN,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.bonding_curve.clone(),
                user: trade_event.user.clone(),
                input_mint: SOL_MINT.to_vec(),
                input_amount: trade_event.sol_amount + protocol_fee + creator_fee,
                output_mint: trade_event.mint.clone(),
                output_amount: trade_event.token_amount,
            })
        }
        Some(pumpfun_pb::instruction::Instruction::Sell(event)) => {
            let accounts = event.accounts.as_ref()?;
            let protocol_fee = trade_event.fee.unwrap_or(0);
            let creator_fee = trade_event.creator_fee.unwrap_or(0);
            Some(pb::Swap {
                protocol: PROTOCOL_PUMPFUN,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.bonding_curve.clone(),
                user: trade_event.user.clone(),
                input_mint: trade_event.mint.clone(),
                input_amount: trade_event.token_amount,
                output_mint: SOL_MINT.to_vec(),
                output_amount: trade_event.sol_amount + protocol_fee + creator_fee,
            })
        }
        _ => None,
    }
}

pub(crate) fn map_pumpfun_amm_transaction(transaction: pumpfun_amm_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| map_pumpfun_amm_swap(&transaction, instruction, instruction_index))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_pumpfun_amm_swap(transaction: &pumpfun_amm_pb::Transaction, instruction: &pumpfun_amm_pb::Instruction, instruction_index: usize) -> Option<pb::Swap> {
    match &instruction.instruction {
        Some(pumpfun_amm_pb::instruction::Instruction::BuyInstruction(event)) => {
            let buy_event = match transaction.instructions.get(instruction_index + 1)?.instruction.as_ref()? {
                pumpfun_amm_pb::instruction::Instruction::BuyEvent(event) => event,
                _ => return None,
            };
            let accounts = event.accounts.as_ref()?;
            Some(pb::Swap {
                protocol: PROTOCOL_PUMPFUN_AMM,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.pool.clone(),
                user: accounts.user.clone(),
                input_mint: accounts.quote_mint.clone(),
                input_amount: buy_event.quote_amount_in,
                output_mint: accounts.base_mint.clone(),
                output_amount: event.base_amount_out,
            })
        }
        Some(pumpfun_amm_pb::instruction::Instruction::SellInstruction(event)) => {
            let sell_event = match transaction.instructions.get(instruction_index + 1)?.instruction.as_ref()? {
                pumpfun_amm_pb::instruction::Instruction::SellEvent(event) => event,
                _ => return None,
            };
            let accounts = event.accounts.as_ref()?;
            Some(pb::Swap {
                protocol: PROTOCOL_PUMPFUN_AMM,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.pool.clone(),
                user: accounts.user.clone(),
                input_mint: accounts.base_mint.clone(),
                input_amount: event.base_amount_in,
                output_mint: accounts.quote_mint.clone(),
                output_amount: sell_event.quote_amount_out,
            })
        }
        _ => None,
    }
}

pub(crate) fn map_raydium_launchpad_transaction(transaction: raydium_launchpad_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| map_raydium_launchpad_swap(&transaction, instruction, instruction_index))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_raydium_launchpad_swap(
    transaction: &raydium_launchpad_pb::Transaction,
    instruction: &raydium_launchpad_pb::Instruction,
    instruction_index: usize,
) -> Option<pb::Swap> {
    let trade_event = match transaction.instructions.get(instruction_index + 1)?.instruction.as_ref()? {
        raydium_launchpad_pb::instruction::Instruction::TradeEvent(event) => event,
        _ => return None,
    };

    match &instruction.instruction {
        Some(raydium_launchpad_pb::instruction::Instruction::BuyExactIn(event)) => {
            let accounts = event.accounts.as_ref()?;
            Some(map_raydium_launchpad_trade(&instruction.program_id, accounts, trade_event, true))
        }
        Some(raydium_launchpad_pb::instruction::Instruction::BuyExactOut(event)) => {
            let accounts = event.accounts.as_ref()?;
            Some(map_raydium_launchpad_trade(&instruction.program_id, accounts, trade_event, true))
        }
        Some(raydium_launchpad_pb::instruction::Instruction::SellExactIn(event)) => {
            let accounts = event.accounts.as_ref()?;
            Some(map_raydium_launchpad_trade(&instruction.program_id, accounts, trade_event, false))
        }
        Some(raydium_launchpad_pb::instruction::Instruction::SellExactOut(event)) => {
            let accounts = event.accounts.as_ref()?;
            Some(map_raydium_launchpad_trade(&instruction.program_id, accounts, trade_event, false))
        }
        _ => None,
    }
}

pub(crate) fn map_raydium_launchpad_trade(
    program_id: &[u8],
    accounts: &raydium_launchpad_pb::TradeAccounts,
    trade_event: &raydium_launchpad_pb::TradeEvent,
    is_buy: bool,
) -> pb::Swap {
    let (input_mint, output_mint) = if is_buy {
        (&accounts.quote_token_mint, &accounts.base_token_mint)
    } else {
        (&accounts.base_token_mint, &accounts.quote_token_mint)
    };

    pb::Swap {
        protocol: PROTOCOL_RAYDIUM_LAUNCHPAD,
        amm: program_id.to_vec(),
        amm_pool: accounts.pool_state.clone(),
        user: accounts.payer.clone(),
        input_mint: input_mint.clone(),
        input_amount: trade_event.amount_in,
        output_mint: output_mint.clone(),
        output_amount: trade_event.amount_out,
    }
}

pub(crate) fn map_meteora_dllm_transaction(transaction: meteora_dllm_pb::Transaction) -> Option<pb::Transaction> {
    let swaps = transaction
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| map_meteora_dllm_swap(&transaction, instruction, instruction_index))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_meteora_dllm_swap(transaction: &meteora_dllm_pb::Transaction, instruction: &meteora_dllm_pb::Instruction, instruction_index: usize) -> Option<pb::Swap> {
    let swap_event = match transaction.instructions.get(instruction_index + 1)?.instruction.as_ref()? {
        meteora_dllm_pb::instruction::Instruction::SwapEvent(event) => event,
        _ => return None,
    };

    match &instruction.instruction {
        Some(meteora_dllm_pb::instruction::Instruction::SwapInstruction(event)) => {
            let accounts = event.accounts.as_ref()?;
            let (input_mint, output_mint) = if swap_event.swap_for_y {
                (&accounts.token_x_mint, &accounts.token_y_mint)
            } else {
                (&accounts.token_y_mint, &accounts.token_x_mint)
            };

            Some(pb::Swap {
                protocol: PROTOCOL_METEORA_DLLM,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.lb_pair.clone(),
                user: accounts.user.clone(),
                input_mint: input_mint.clone(),
                input_amount: swap_event.amount_in,
                output_mint: output_mint.clone(),
                output_amount: swap_event.amount_out,
            })
        }
        _ => None,
    }
}

pub(crate) fn map_meteora_daam_transaction(transaction: meteora_daam_pb::Transaction) -> Option<pb::Transaction> {
    if transaction.logs.len() != transaction.instructions.len() {
        return None;
    }

    let swaps = transaction
        .instructions
        .iter()
        .zip(transaction.logs.iter())
        .filter_map(|(instruction, log)| map_meteora_daam_swap(instruction, log))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_meteora_daam_swap(instruction: &meteora_daam_pb::Instruction, log: &meteora_daam_pb::Log) -> Option<pb::Swap> {
    let traded_event = match &log.log {
        Some(meteora_daam_pb::log::Log::Swap(event)) => event,
        _ => return None,
    };

    match &instruction.instruction {
        Some(meteora_daam_pb::instruction::Instruction::Swap(event)) => {
            let accounts = event.accounts.as_ref()?;
            let (input_mint, output_mint) = if traded_event.trade_direction == 0 {
                (&accounts.token_a_mint, &accounts.token_b_mint)
            } else {
                (&accounts.token_b_mint, &accounts.token_a_mint)
            };
            let output_amount = traded_event.result.as_ref().map(|result| result.output_amount).unwrap_or_default();

            Some(pb::Swap {
                protocol: PROTOCOL_METEORA_DAAM,
                amm: instruction.program_id.clone(),
                amm_pool: accounts.pool.clone(),
                user: accounts.payer.clone(),
                input_mint: input_mint.clone(),
                input_amount: traded_event.actual_amount_in,
                output_mint: output_mint.clone(),
                output_amount,
            })
        }
        _ => None,
    }
}

pub(crate) fn map_raydium_clmm_transaction(transaction: raydium_clmm_pb::Transaction) -> Option<pb::Transaction> {
    if transaction.logs.len() != transaction.instructions.len() {
        return None;
    }

    let swaps = transaction
        .instructions
        .iter()
        .zip(transaction.logs.iter())
        .filter_map(|(instruction, log)| map_raydium_clmm_swap(instruction, log))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_raydium_clmm_swap(instruction: &raydium_clmm_pb::Instruction, log: &raydium_clmm_pb::Log) -> Option<pb::Swap> {
    let traded_event = match &log.log {
        Some(raydium_clmm_pb::log::Log::Swap(event)) => event,
        _ => return None,
    };

    match &instruction.instruction {
        Some(raydium_clmm_pb::instruction::Instruction::Swap(event)) => match &event.accounts {
            Some(raydium_clmm_pb::swap_instruction::Accounts::V2Accounts(accounts)) => {
                let (input_amount, output_amount) = if traded_event.zero_for_one {
                    (traded_event.amount_0, traded_event.amount_1)
                } else {
                    (traded_event.amount_1, traded_event.amount_0)
                };

                Some(pb::Swap {
                    protocol: PROTOCOL_RAYDIUM_CLMM,
                    amm: instruction.program_id.clone(),
                    amm_pool: accounts.pool_state.clone(),
                    user: accounts.payer.clone(),
                    input_mint: accounts.input_vault_mint.clone(),
                    input_amount,
                    output_mint: accounts.output_vault_mint.clone(),
                    output_amount,
                })
            }
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn map_raydium_cpmm_transaction(transaction: raydium_cpmm_pb::Transaction) -> Option<pb::Transaction> {
    if transaction.logs.len() != transaction.instructions.len() {
        return None;
    }

    let swaps = transaction
        .instructions
        .iter()
        .zip(transaction.logs.iter())
        .filter_map(|(instruction, log)| map_raydium_cpmm_swap(instruction, log))
        .collect::<Vec<_>>();

    if swaps.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        signature: transaction.signature,
        fee_payer: transaction.fee_payer,
        signers: transaction.signers,
        fee: transaction.fee,
        compute_units_consumed: transaction.compute_units_consumed,
        swaps,
    })
}

pub(crate) fn map_raydium_cpmm_swap(instruction: &raydium_cpmm_pb::Instruction, log: &raydium_cpmm_pb::Log) -> Option<pb::Swap> {
    let traded_event = match &log.log {
        Some(raydium_cpmm_pb::log::Log::Swap(event)) => event,
        _ => return None,
    };

    let accounts = match &instruction.instruction {
        Some(raydium_cpmm_pb::instruction::Instruction::SwapBaseInput(event)) => event.accounts.as_ref(),
        Some(raydium_cpmm_pb::instruction::Instruction::SwapBaseOutput(event)) => event.accounts.as_ref(),
        _ => None,
    }?;

    Some(pb::Swap {
        protocol: PROTOCOL_RAYDIUM_CPMM,
        amm: instruction.program_id.clone(),
        amm_pool: accounts.pool_state.clone(),
        user: accounts.payer.clone(),
        input_mint: accounts.input_token_mint.clone(),
        input_amount: traded_event.input_amount,
        output_mint: accounts.output_token_mint.clone(),
        output_amount: traded_event.output_amount,
    })
}
