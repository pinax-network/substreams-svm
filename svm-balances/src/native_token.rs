use std::collections::HashMap;

use proto::pb::solana::native::token::v1 as pb;
use substreams::pb::substreams::Clock;
use substreams_solana::base58;

use crate::set_clock;

pub fn process_events(tables: &mut substreams_database_change::tables::Tables, clock: &Clock, events: &pb::Events) {
    let mut system_post_balances_per_block = HashMap::new();
    let mut system_pre_balances_per_block = HashMap::new();
    for (transaction_index, transaction) in events.transactions.iter().enumerate() {
        // Native Token Balances
        // Keep first pre balance and last post balance per account
        for (i, post_balance) in transaction.post_balances.iter().enumerate() {
            let key = post_balance.account.as_slice();
            system_post_balances_per_block.insert(key, (post_balance, transaction, transaction_index, i));
        }
        for (i, pre_balance) in transaction.pre_balances.iter().enumerate() {
            let key = pre_balance.account.as_slice();
            if !system_pre_balances_per_block.contains_key(&key) {
                system_pre_balances_per_block.insert(key, (pre_balance, transaction, transaction_index, i));
            }
        }
    }
    let mut skipped = 0;
    for (post_balance, _, _, _) in system_post_balances_per_block.values() {
        if let Some((pre_balance, _, _, _)) = system_pre_balances_per_block.get(&(post_balance.account.as_slice())) {
            // Roughly 80% of balances don't change - no need to include these - just skip them
            if pre_balance.amount == post_balance.amount {
                skipped += 1;
                continue;
            }
        }
        handle_balances(tables, clock, post_balance);
    }
    substreams::log::info!("Skipped {} out of {} native token balances", skipped, system_post_balances_per_block.len());
}

fn handle_balances(
    tables: &mut substreams_database_change::tables::Tables,
    clock: &Clock,
    data: &pb::Balance
) {
    let account = base58::encode(&data.account);
    let row = tables
        .upsert_row("native_balances", account.clone())
        .set("account", account)
        .set("amount", data.amount);

    set_clock(clock, row);
}
