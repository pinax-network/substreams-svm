use std::collections::HashMap;

use substreams_solana::{
    base58,
    pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta},
};

pub(crate) struct TokenMintLookup {
    mints: HashMap<Vec<u8>, Vec<u8>>,
}

impl TokenMintLookup {
    pub(crate) fn new(tx: &ConfirmedTransaction, tx_meta: &TransactionStatusMeta) -> Self {
        let accounts = tx.resolved_accounts();
        let mut mints = HashMap::new();

        for balance in tx_meta.pre_token_balances.iter().chain(tx_meta.post_token_balances.iter()) {
            let Some(account) = accounts.get(balance.account_index as usize) else {
                continue;
            };
            let Ok(mint) = base58::decode(&balance.mint) else {
                continue;
            };
            mints.insert((*account).clone(), mint);
        }

        Self { mints }
    }

    pub(crate) fn mint_for(&self, token_account: &[u8]) -> Option<Vec<u8>> {
        self.mints.get(token_account).cloned()
    }
}
