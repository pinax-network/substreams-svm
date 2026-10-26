//! Identity from typed successful pool initialization instructions.
use crate::{
    decode,
    solana::{base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction},
    Candidate,
};
use std::collections::{btree_map::Entry, BTreeMap};
mod additional_venues;
mod aldrin;
mod other_venues;
mod raydium_meteora;
mod token_swaps;

/// Token account identity supplied by the caller's transaction metadata. This
/// is context for initializers whose accounts contain vaults but omit mints.
/// The raw decoder cannot independently authenticate this context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenAccountMetadata {
    pub address: [u8; 32],
    pub mint: [u8; 32],
    pub decimals: Option<u8>,
    pub token_program: Option<[u8; 32]>,
}

/// Decode initialization identity from borrowed instruction data and resolved
/// accounts. The result is unverified until the caller proves successful
/// transaction and invocation ancestry. Missing decimals remain absent.
/// SPL Token Swap, Saros and Aldrin require the contextual variant below because their
/// initialization accounts do not contain the underlying mints.
pub fn decode_pool_initialization(program_id: &[u8], instruction_data: &[u8], accounts: &[&[u8]]) -> Option<Candidate> {
    decode::with_instruction(program_id, instruction_data, accounts, from_view)
}

/// Decode with at most 512 token-account identities supplied by a native
/// consumer. Conflicting identities for one account cannot identify a mint.
/// This still returns an unverified candidate, not execution proof.
pub fn decode_pool_initialization_with_token_accounts(
    program_id: &[u8],
    instruction_data: &[u8],
    accounts: &[&[u8]],
    metadata: &[TokenAccountMetadata],
) -> Option<Candidate> {
    if metadata.len() > 512 {
        return None;
    }
    let mut tokens = TokenAccounts::default();
    for item in metadata {
        tokens.insert(item.clone());
    }
    decode::with_instruction(program_id, instruction_data, accounts, |ix| from_view_with_tokens(ix, &tokens))
}

pub(crate) fn supports(program_id: &[u8]) -> bool {
    other_venues::supports(program_id)
        || raydium_meteora::supports(program_id)
        || additional_venues::supports(program_id)
        || token_swaps::supports(program_id)
        || aldrin::supports(program_id)
}
pub(crate) fn needs_token_accounts(program_id: &[u8]) -> bool {
    token_swaps::needs_token_accounts(program_id) || aldrin::supports(program_id)
}
pub(crate) fn from_view(ix: &InstructionView<'_>) -> Option<Candidate> {
    from_view_with_tokens(ix, &TokenAccounts::default())
}
pub(crate) fn from_view_with_tokens(ix: &InstructionView<'_>, tokens: &TokenAccounts) -> Option<Candidate> {
    if ix.data().len() > 2048 || ix.accounts().len() > 255 {
        return None;
    }
    let candidate = other_venues::decode(ix)
        .or_else(|| raydium_meteora::decode(ix))
        .or_else(|| additional_venues::decode(ix))
        .or_else(|| token_swaps::decode(ix, tokens))
        .or_else(|| aldrin::decode(ix, tokens))?;
    decode::validate(candidate)
}

#[derive(Default)]
pub(crate) struct TokenAccounts(BTreeMap<[u8; 32], Option<TokenAccountMetadata>>);
impl TokenAccounts {
    pub(super) fn get(&self, address: &[u8; 32]) -> Option<&TokenAccountMetadata> {
        self.0.get(address)?.as_ref()
    }

    fn insert(&mut self, value: TokenAccountMetadata) {
        match self.0.entry(value.address) {
            Entry::Vacant(entry) => {
                entry.insert(Some(value));
            }
            Entry::Occupied(mut entry) => {
                let Some(previous) = entry.get_mut() else {
                    return;
                };
                if previous.mint != value.mint
                    || matches!((previous.decimals, value.decimals), (Some(a), Some(b)) if a != b)
                    || matches!((previous.token_program, value.token_program), (Some(a), Some(b)) if a != b)
                {
                    *entry.get_mut() = None;
                } else {
                    previous.decimals = previous.decimals.or(value.decimals);
                    previous.token_program = previous.token_program.or(value.token_program);
                }
            }
        }
    }

    pub(crate) fn from_transaction(tx: &ConfirmedTransaction) -> Self {
        let mut out = Self::default();
        let Some(meta) = tx.meta.as_option() else {
            return out;
        };
        let Some(message) = tx.transaction.as_option().and_then(|tx| tx.message.as_option()) else {
            return out;
        };
        // Block limits and structural validation precede this call. Resolve
        // each original account index once; never infer mints from amounts or
        // from another user's token account. Post/pre conflicts remain unknown.
        for balance in meta.pre_token_balances.iter().chain(&meta.post_token_balances) {
            let mut index = balance.account_index as usize;
            let account = if index < message.account_keys.len() {
                message.account_keys.get(index)
            } else {
                index -= message.account_keys.len();
                if index < meta.loaded_writable_addresses.len() {
                    meta.loaded_writable_addresses.get(index)
                } else {
                    meta.loaded_readonly_addresses.get(index - meta.loaded_writable_addresses.len())
                }
            };
            let Some(address) = account.and_then(|key| key.as_slice().try_into().ok()) else {
                continue;
            };
            let parse = || {
                let mint = parse_key(&balance.mint)?;
                let token_program = if balance.program_id.is_empty() {
                    None
                } else {
                    Some(parse_key(&balance.program_id)?)
                };
                let decimals = match balance.ui_token_amount.as_option() {
                    Some(amount) => Some(u8::try_from(amount.decimals).ok()?),
                    None => None,
                };
                Some(TokenAccountMetadata {
                    address,
                    mint,
                    decimals,
                    token_program,
                })
            };
            match parse() {
                Some(value) => out.insert(value),
                None => {
                    out.0.insert(address, None);
                }
            }
        }
        out
    }
}
fn parse_key(text: &str) -> Option<[u8; 32]> {
    if text.is_empty() || text.len() > 44 {
        return None;
    }
    base58::decode(text).ok()?.try_into().ok()
}
