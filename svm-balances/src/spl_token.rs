use std::collections::{HashMap, HashSet};

use proto::pb::solana::spl::token::v1 as pb;
use substreams::pb::substreams::Clock;
use substreams_solana::base58;

use crate::set_clock;

pub fn process_events(tables: &mut substreams_database_change::tables::Tables, clock: &Clock, events: &pb::Events) {
    // A token account closed in a transaction has no post-balance in it, so without this its last
    // positive amount would stay current forever. Each transaction's closed accounts get a
    // post-balance of 0, which the ReplacingMergeTree deletes on merge (`is_deleted = amount = 0`).
    let closed_per_transaction: Vec<Vec<pb::TokenBalance>> = events.transactions.iter().map(closed_token_balances).collect();

    // Only keep last balance change per block: the first pre-balance and the last post-balance of
    // each account and mint, in transaction order, a closed account's zero being its post-balance.
    let mut post_token_balances_per_block: HashMap<(&[u8], &[u8]), &pb::TokenBalance> = HashMap::new();
    let mut pre_token_balances_per_block: HashMap<(&[u8], &[u8]), &pb::TokenBalance> = HashMap::new();
    for (transaction, closed) in events.transactions.iter().zip(&closed_per_transaction) {
        for balance in &transaction.pre_token_balances {
            pre_token_balances_per_block.entry(balance_key(balance)).or_insert(balance);
        }
        for balance in transaction.post_token_balances.iter().chain(closed) {
            post_token_balances_per_block.insert(balance_key(balance), balance);
        }
    }
    let mut skipped = 0;
    for post_balance in post_token_balances_per_block.values() {
        // if balance not changed in the block - no need to include it - skip it
        if let Some(pre_balance) = pre_token_balances_per_block.get(&balance_key(post_balance)) {
            if pre_balance.amount == post_balance.amount {
                skipped += 1;
                continue;
            }
        }
        handle_token_balances(tables, clock, post_balance);
    }
    substreams::log::info!("Skipped {} out of {} spl token balances", skipped, post_token_balances_per_block.len());
}

fn balance_key(balance: &pb::TokenBalance) -> (&[u8], &[u8]) {
    (balance.account.as_slice(), balance.mint.as_slice())
}

/// The token accounts a transaction closed, each as a post-balance of 0: an account in the
/// transaction's pre-balances but not in its post-balances no longer exists after it. Only
/// successful transactions reach here, and an account re-created later in the block gets its
/// new post-balance from that later transaction.
fn closed_token_balances(transaction: &pb::Transaction) -> Vec<pb::TokenBalance> {
    let open: HashSet<(&[u8], &[u8])> = transaction.post_token_balances.iter().map(balance_key).collect();
    transaction
        .pre_token_balances
        .iter()
        .filter(|balance| !open.contains(&balance_key(balance)))
        .map(|balance| pb::TokenBalance { amount: 0, ..balance.clone() })
        .collect()
}

fn handle_token_balances(tables: &mut substreams_database_change::tables::Tables, clock: &Clock, data: &pb::TokenBalance) {
    let mint = base58::encode(&data.mint);
    let account = base58::encode(&data.account);
    let key = [("account", account.clone()), ("mint", mint.clone())];
    let row = tables
        .upsert_row("balances", key)
        .set("program_id", base58::encode(&data.program_id))
        .set("mint", mint)
        .set("account", account)
        .set("amount", data.amount)
        .set("decimals", data.decimals);

    set_clock(clock, row);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use substreams::buffa::MessageField;
    use substreams_database_change::tables::Tables;

    use super::*;

    const MINT: [u8; 32] = [9; 32];

    fn balance(account: u8, amount: u64) -> pb::TokenBalance {
        pb::TokenBalance {
            program_id: vec![1; 32],
            account: vec![account; 32],
            mint: MINT.to_vec(),
            amount,
            decimals: 6,
        }
    }

    fn transaction(pre: Vec<pb::TokenBalance>, post: Vec<pb::TokenBalance>) -> pb::Transaction {
        pb::Transaction {
            pre_token_balances: pre,
            post_token_balances: post,
            ..Default::default()
        }
    }

    /// The `balances` rows of a block, by account: (amount, block_num).
    fn balances_rows(transactions: Vec<pb::Transaction>) -> BTreeMap<String, (String, String)> {
        let mut clock = Clock {
            id: "block-77".into(),
            number: 77,
            timestamp: MessageField::some(Default::default()),
        };
        clock.timestamp.get_or_insert_default().seconds = 1_700_000_000;
        let mut tables = Tables::new();
        process_events(&mut tables, &clock, &pb::Events { transactions });
        tables
            .to_database_changes()
            .table_changes
            .iter()
            .map(|row| {
                assert_eq!(row.table, "balances");
                let fields: BTreeMap<_, _> = row.fields.iter().map(|field| (field.name.as_str(), field.value.clone())).collect();
                assert_eq!(fields["mint"], base58::encode(MINT));
                assert_eq!(fields["program_id"], base58::encode([1; 32]));
                assert_eq!(fields["decimals"], "6");
                (fields["account"].clone(), (fields["amount"].clone(), fields["block_num"].clone()))
            })
            .collect()
    }

    #[test]
    fn a_closed_account_gets_a_zero_balance() {
        let rows = balances_rows(vec![
            // Account 1 is emptied and closed in one transaction; account 2 transfers out and stays open.
            transaction(vec![balance(1, 500), balance(2, 300)], vec![balance(2, 100)]),
            // Account 2 is closed in a later transaction of the block; account 3 was already empty.
            transaction(vec![balance(2, 100), balance(3, 0)], vec![]),
            // Account 4 is closed, then re-created with a new balance; account 5 does not change.
            transaction(vec![balance(4, 50), balance(5, 10)], vec![balance(5, 10)]),
            transaction(vec![], vec![balance(4, 70)]),
        ]);
        let expected: BTreeMap<String, (String, String)> = [
            (base58::encode([1; 32]), ("0".to_string(), "77".to_string())),
            (base58::encode([2; 32]), ("0".to_string(), "77".to_string())),
            (base58::encode([4; 32]), ("70".to_string(), "77".to_string())),
        ]
        .into_iter()
        .collect();
        // Account 3 was 0 before and after its close, and account 5 is unchanged: neither is written.
        assert_eq!(rows, expected);
    }

    #[test]
    fn an_account_open_at_the_end_of_the_block_keeps_its_post_balance() {
        let rows = balances_rows(vec![
            transaction(vec![balance(1, 500)], vec![balance(1, 200)]),
            transaction(vec![balance(1, 200)], vec![balance(1, 200)]),
        ]);
        assert_eq!(rows[&base58::encode([1; 32])], ("200".to_string(), "77".to_string()));
        assert_eq!(rows.len(), 1);
    }

    const FIXTURES: [(&str, &[u8]); 5] = [
        ("349019252", include_bytes!("../../spl/transfers/tests/fixtures/solana-mainnet-349019252.pb.gz")),
        ("350000009", include_bytes!("../../spl/transfers/tests/fixtures/solana-mainnet-350000009.pb.gz")),
        ("350000010", include_bytes!("../../spl/transfers/tests/fixtures/solana-mainnet-350000010.pb.gz")),
        ("350000026", include_bytes!("../../spl/transfers/tests/fixtures/solana-mainnet-350000026.pb.gz")),
        (
            "461335700",
            include_bytes!("../../spl/transfers/tests/fixtures/rpc/solana-mainnet-461335700.pb.gz"),
        ),
    ];

    fn is_spl_token_program(program_id: &[u8]) -> bool {
        program_id == spl_token_common::SOLANA_TOKEN_PROGRAM_KEG || program_id == spl_token_common::SOLANA_TOKEN_PROGRAM_ZQB
    }

    /// The `balances` rows of a block's events, by (account, mint): amount.
    fn rows_by_key(events: &pb::Events) -> BTreeMap<(String, String), String> {
        let clock = Clock {
            id: "fixture".into(),
            number: 1,
            timestamp: MessageField::some(Default::default()),
        };
        let mut tables = Tables::new();
        process_events(&mut tables, &clock, events);
        tables
            .to_database_changes()
            .table_changes
            .iter()
            .map(|row| {
                let fields: BTreeMap<_, _> = row.fields.iter().map(|field| (field.name.as_str(), field.value.clone())).collect();
                ((fields["account"].clone(), fields["mint"].clone()), fields["amount"].clone())
            })
            .collect()
    }

    /// Mainnet blocks: an account whose last appearance in the block is a close gets a zero row
    /// unless it was already empty, and every other row follows the former per-block rule.
    #[test]
    fn closed_accounts_in_mainnet_blocks_get_zero_rows() {
        use buffa::Message;
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut closes_in_fixtures = 0;
        for (slot, fixture) in FIXTURES {
            let mut bytes = Vec::new();
            GzDecoder::new(fixture).read_to_end(&mut bytes).unwrap();
            let block = substreams_solana::pb::sf::solana::r#type::v1::Block::decode_from_slice(&bytes).unwrap();
            let events = spl_token_common::process_block(&block, is_spl_token_program).unwrap();

            // The rule restated: first pre-balance and last post-balance per account and mint, a
            // close counting as a post-balance of 0 (None), and unchanged amounts are not written.
            let mut first_pre: HashMap<(&[u8], &[u8]), u64> = HashMap::new();
            let mut last_post: HashMap<(&[u8], &[u8]), Option<u64>> = HashMap::new();
            for transaction in &events.transactions {
                let open: HashSet<_> = transaction.post_token_balances.iter().map(balance_key).collect();
                for balance in &transaction.pre_token_balances {
                    first_pre.entry(balance_key(balance)).or_insert(balance.amount);
                    if !open.contains(&balance_key(balance)) {
                        last_post.insert(balance_key(balance), None);
                    }
                }
                for balance in &transaction.post_token_balances {
                    last_post.insert(balance_key(balance), Some(balance.amount));
                }
            }
            let mut expected = BTreeMap::new();
            let (mut closes, mut zero_rows) = (0, 0);
            for (key, post) in &last_post {
                let amount = post.unwrap_or(0);
                closes += post.is_none() as usize;
                if first_pre.get(key) == Some(&amount) {
                    continue;
                }
                zero_rows += post.is_none() as usize;
                expected.insert((base58::encode(key.0), base58::encode(key.1)), amount.to_string());
            }
            let rows = rows_by_key(&events);
            assert_eq!(rows, expected, "block {slot}");
            eprintln!(
                "block {slot}: {} transactions, {} balances rows, {closes} closed accounts, {zero_rows} zero rows for them",
                events.transactions.len(),
                rows.len()
            );
            closes_in_fixtures += closes;
        }
        assert!(closes_in_fixtures > 0, "no closed token account in the fixtures");
    }
}
