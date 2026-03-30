use std::collections::HashMap;

use proto::pb::solana::spl::token::v1 as pb;
use substreams::{pb::substreams::Clock};
use substreams_solana::base58;

use crate::set_clock;

pub fn process_events(tables: &mut substreams_database_change::tables::Tables, clock: &Clock, events: &pb::Events) {
    // Only keep last balance change per block
    let mut post_token_balances_per_block = HashMap::new();
    let mut pre_token_balances_per_block = HashMap::new();
    for (transaction_index, transaction) in events.transactions.iter().enumerate() {
        // Keep first pre balance and last post balance per account
        for (i, balance) in transaction.pre_token_balances.iter().enumerate() {
            let key = (balance.account.as_slice(), balance.mint.as_slice());
            if !pre_token_balances_per_block.contains_key(&key) {
                pre_token_balances_per_block.insert(key, (balance, transaction, transaction_index, i));
            }
        }
        for (i, balance) in transaction.post_token_balances.iter().enumerate() {
            let key = (balance.account.as_slice(), balance.mint.as_slice());
            post_token_balances_per_block.insert(key, (balance, transaction, transaction_index, i));
        }
    }
    let mut skipped = 0;
    for (post_balance,_,_,_) in post_token_balances_per_block.values() {
        // if balance not changed in the block - no need to include it - skip it
        if let Some((pre_balance, _, _, _)) = pre_token_balances_per_block.get(&(post_balance.account.as_slice(), post_balance.mint.as_slice())) {
            if pre_balance.amount == post_balance.amount {
                skipped += 1;
                continue;
            }
        }
        handle_token_balances(tables, clock, post_balance);
    }
    substreams::log::info!("Skipped {} out of {} spl token balances", skipped, post_token_balances_per_block.len());
}

fn handle_token_balances(
    tables: &mut substreams_database_change::tables::Tables,
    clock: &Clock,
    data: &pb::TokenBalance,
) {
    let mint = base58::encode(&data.mint);
    let account = base58::encode(&data.account);
    let key = [("account", account.clone()), ("mint", mint.clone())];
    let row = tables
        .create_row("balances", key)
        .set("program_id", base58::encode(&data.program_id))
        .set("mint", mint)
        .set("account", account)
        .set("amount", data.amount)
        .set("decimals", data.decimals);

    set_clock(clock, row);
}
