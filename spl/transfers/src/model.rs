//! Borrowed view of a Firehose `sf.solana.type.v1.Block`.
//!
//! The view only holds references into a block the caller already decoded, so
//! any generated Rust model can be projected without re-encoding it: prost 0.13
//! with `Vec<u8>` fields (substreams-solana) or prost 0.14 with `bytes::Bytes`
//! fields (firehose-protos) alike. [`block_view!`](crate::block_view) handles
//! Prost, and [`block_view_buffa!`](crate::block_view_buffa) handles Buffa's
//! presence-aware `MessageField` without adding a protobuf runtime dependency.
//!
//! Fields mirror the protobuf names. Presence (`Option`) is kept where the
//! protobuf message field is optional so validation can reject missing parts
//! instead of guessing.

/// `sf.solana.type.v1.Block`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block<'a> {
    pub slot: u64,
    pub parent_slot: u64,
    /// Base58 block hash, as carried by Firehose.
    pub blockhash: &'a str,
    /// Base58 parent block hash, as carried by Firehose.
    pub previous_blockhash: &'a str,
    /// `block_time.timestamp` in Unix seconds; `None` when the block has no time.
    pub block_time: Option<i64>,
    /// `block_height.block_height`; `None` when absent.
    pub block_height: Option<u64>,
    /// Every transaction of the block in original order (votes and failures included).
    pub transactions: Vec<Transaction<'a>>,
}

/// `sf.solana.type.v1.ConfirmedTransaction`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transaction<'a> {
    /// `transaction.signatures`; empty when `transaction` is absent.
    pub signatures: Vec<&'a [u8]>,
    /// `transaction.message`; `None` when `transaction` or its message is absent.
    pub message: Option<Message<'a>>,
    pub meta: Option<Meta<'a>>,
}

/// `sf.solana.type.v1.Message`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message<'a> {
    pub header: Option<MessageHeader>,
    /// Static account keys.
    pub account_keys: Vec<&'a [u8]>,
    /// Top-level (outer) instructions.
    pub instructions: Vec<Instruction<'a>>,
    pub address_table_lookups: Vec<AddressTableLookup<'a>>,
}

/// `sf.solana.type.v1.MessageHeader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessageHeader {
    pub num_required_signatures: u32,
    pub num_readonly_signed_accounts: u32,
    pub num_readonly_unsigned_accounts: u32,
}

/// `sf.solana.type.v1.MessageAddressTableLookup`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddressTableLookup<'a> {
    pub account_key: &'a [u8],
    pub writable_indexes: &'a [u8],
    pub readonly_indexes: &'a [u8],
}

/// A compiled (outer) or inner instruction. Account and program references are
/// indexes into the transaction's resolved keys: static keys, then loaded
/// writable, then loaded readonly addresses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Instruction<'a> {
    pub program_id_index: u32,
    pub accounts: &'a [u8],
    pub data: &'a [u8],
    /// `InnerInstruction.stack_height`; always `None` for outer instructions,
    /// which Firehose does not record a height for.
    pub stack_height: Option<u32>,
}

/// `sf.solana.type.v1.TransactionStatusMeta`, restricted to what projection needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta<'a> {
    /// `err` is present.
    pub failed: bool,
    pub loaded_writable_addresses: Vec<&'a [u8]>,
    pub loaded_readonly_addresses: Vec<&'a [u8]>,
    pub inner_instructions: Vec<InnerInstructions<'a>>,
    pub pre_token_balances: Vec<TokenBalance<'a>>,
    pub post_token_balances: Vec<TokenBalance<'a>>,
}

/// `sf.solana.type.v1.InnerInstructions`: the inner instructions recorded under
/// the outer instruction at `index`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InnerInstructions<'a> {
    pub index: u32,
    pub instructions: Vec<Instruction<'a>>,
}

/// `sf.solana.type.v1.TokenBalance`, without the amount.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenBalance<'a> {
    pub account_index: u32,
    /// Base58 mint.
    pub mint: &'a str,
    /// Base58 owner wallet; empty when the producer did not record it.
    pub owner: &'a str,
    /// Base58 token program; empty when the producer did not record it.
    pub program_id: &'a str,
}

/// Generates `fn $name(block: &pb::Block) -> spl_transfers::model::Block<'_>`
/// for a generated `sf.solana.type.v1` module.
///
/// The generated code only reads fields, so it accepts any prost version and
/// both `Vec<u8>` and `bytes::Bytes` byte fields. Nothing is copied or encoded.
///
/// ```ignore
/// // prost 0.13 model from substreams-solana
/// spl_transfers::block_view!(pub fn view_block, substreams_solana::pb::sf::solana::r#type::v1);
/// // prost 0.14 model with `Bytes` fields from firehose-protos
/// spl_transfers::block_view!(fn view_block, firehose_protos::sf::solana::r#type::v1);
///
/// let projection = spl_transfers::project(&view_block(&block), spl_transfers::Coverage::TRANSFER_CHECKED)?;
/// ```
///
/// Write the module path without a leading `::`.
#[macro_export]
macro_rules! block_view {
    ($vis:vis fn $name:ident, $($pb:ident)::+) => {
        $crate::__block_view!($vis fn $name, $($pb)::+; as_ref, is_some);
    };
}

/// Generates a borrowed block view for Buffa's `sf.solana.type.v1` model,
/// including `substreams-solana-core` 0.16.
///
/// Missing messages remain absent instead of dereferencing Buffa's default
/// instance. Byte fields are borrowed, never copied or re-encoded. This macro
/// does not make the native projector depend on Buffa or Substreams.
///
/// Returns `Result<model::Block<'_>, UnsupportedMessage>`: legacy messages
/// (`version: None`), v0 (`Some(0)`) without inline transaction configuration,
/// and v1 (`Some(1)`) without address table lookups, with or without its inline
/// compute budget, are covered. Newer message shapes fail before their fields
/// can be discarded.
///
/// ```ignore
/// spl_transfers::block_view_buffa!(pub fn view_block, substreams_solana::pb::sf::solana::r#type::v1);
/// ```
#[macro_export]
macro_rules! block_view_buffa {
    ($vis:vis fn $name:ident, $($pb:ident)::+) => {
        $vis fn $name(block: &$($pb)::+::Block) -> ::core::result::Result<$crate::model::Block<'_>, $crate::UnsupportedMessage> {
            for (transaction_index, confirmed) in block.transactions.iter().enumerate() {
                if let Some(unsupported) = $crate::__unsupported_message!(transaction_index, confirmed) {
                    return Err(unsupported);
                }
            }
            $crate::__block_view!(fn borrow_block, $($pb)::+; as_option, is_set);
            Ok(borrow_block(block))
        }
    };
}

/// Lenient variant of [`block_view_buffa!`] for production Substreams modules
/// that must not halt on a message shape this crate does not cover.
///
/// Returns the complete borrowed view, so original transaction indexes are
/// unchanged, together with every transaction whose message
/// [`block_view_buffa!`] would reject. Callers must skip those transactions and
/// validate the rest with [`validate_block_except`](crate::validate_block_except).
/// [`project`](crate::project) does not accept this policy; use the
/// fail-closed adapter for native projection.
///
/// ```ignore
/// spl_transfers::block_view_buffa_lenient!(fn view_block, substreams_solana::pb::sf::solana::r#type::v1);
///
/// let (view, unsupported) = view_block(&block);
/// spl_transfers::validate_block_except(&view, &unsupported)?;
/// ```
#[macro_export]
macro_rules! block_view_buffa_lenient {
    ($vis:vis fn $name:ident, $($pb:ident)::+) => {
        $vis fn $name(block: &$($pb)::+::Block) -> ($crate::model::Block<'_>, ::std::vec::Vec<$crate::UnsupportedMessage>) {
            let unsupported = block
                .transactions
                .iter()
                .enumerate()
                .filter_map(|(transaction_index, confirmed)| $crate::__unsupported_message!(transaction_index, confirmed))
                .collect();
            $crate::__block_view!(fn borrow_block, $($pb)::+; as_option, is_set);
            (borrow_block(block), unsupported)
        }
    };
}

// The message shapes the Buffa adapters do not cover: explicit versions above
// v1, contradictory version metadata, inline transaction configuration outside
// v1, or address table lookups in v1. A v1 message resolves every account from
// its static keys; its inline compute budget is not read by any projection.
#[doc(hidden)]
#[macro_export]
macro_rules! __unsupported_message {
    ($transaction_index:expr, $confirmed:expr) => {
        $confirmed
            .transaction
            .as_option()
            .and_then(|transaction| transaction.message.as_option())
            .filter(|message| match message.version {
                None => message.transaction_config.is_set(),
                Some(0) => !message.versioned || message.transaction_config.is_set(),
                Some(1) => !message.versioned || !message.address_table_lookups.is_empty(),
                Some(_) => true,
            })
            .map(|message| $crate::UnsupportedMessage {
                transaction_index: $transaction_index,
                version: message.version,
                versioned: message.versioned,
                inline_configuration: message.transaction_config.is_set(),
                address_table_lookups: message.address_table_lookups.len(),
            })
    };
}

// Keep both adapters on the same field mapping; only message presence differs.
#[doc(hidden)]
#[macro_export]
macro_rules! __block_view {
    ($vis:vis fn $name:ident, $($pb:ident)::+; $as_option:ident, $is_present:ident) => {
        $vis fn $name(block: &$($pb)::+::Block) -> $crate::model::Block<'_> {
            fn bytes<T: ::core::convert::AsRef<[u8]> + ?Sized>(value: &T) -> &[u8] {
                value.as_ref()
            }
            $crate::model::Block {
                slot: block.slot,
                parent_slot: block.parent_slot,
                blockhash: &block.blockhash,
                previous_blockhash: &block.previous_blockhash,
                block_time: block.block_time.$as_option().map(|time| time.timestamp),
                block_height: block.block_height.$as_option().map(|height| height.block_height),
                transactions: block
                    .transactions
                    .iter()
                    .map(|confirmed| {
                        let transaction = confirmed.transaction.$as_option();
                        $crate::model::Transaction {
                            signatures: transaction
                                .map(|transaction| transaction.signatures.iter().map(|signature| bytes(signature)).collect())
                                .unwrap_or_default(),
                            message: transaction
                                .and_then(|transaction| transaction.message.$as_option())
                                .map(|message| $crate::model::Message {
                                    header: message.header.$as_option().map(|header| $crate::model::MessageHeader {
                                        num_required_signatures: header.num_required_signatures,
                                        num_readonly_signed_accounts: header.num_readonly_signed_accounts,
                                        num_readonly_unsigned_accounts: header.num_readonly_unsigned_accounts,
                                    }),
                                    account_keys: message.account_keys.iter().map(|key| bytes(key)).collect(),
                                    instructions: message
                                        .instructions
                                        .iter()
                                        .map(|instruction| $crate::model::Instruction {
                                            program_id_index: instruction.program_id_index,
                                            accounts: bytes(&instruction.accounts),
                                            data: bytes(&instruction.data),
                                            stack_height: None,
                                        })
                                        .collect(),
                                    address_table_lookups: message
                                        .address_table_lookups
                                        .iter()
                                        .map(|lookup| $crate::model::AddressTableLookup {
                                            account_key: bytes(&lookup.account_key),
                                            writable_indexes: bytes(&lookup.writable_indexes),
                                            readonly_indexes: bytes(&lookup.readonly_indexes),
                                        })
                                        .collect(),
                                }),
                            meta: confirmed.meta.$as_option().map(|meta| $crate::model::Meta {
                                failed: meta.err.$is_present(),
                                loaded_writable_addresses: meta.loaded_writable_addresses.iter().map(|key| bytes(key)).collect(),
                                loaded_readonly_addresses: meta.loaded_readonly_addresses.iter().map(|key| bytes(key)).collect(),
                                inner_instructions: meta
                                    .inner_instructions
                                    .iter()
                                    .map(|inner| $crate::model::InnerInstructions {
                                        index: inner.index,
                                        instructions: inner
                                            .instructions
                                            .iter()
                                            .map(|instruction| $crate::model::Instruction {
                                                program_id_index: instruction.program_id_index,
                                                accounts: bytes(&instruction.accounts),
                                                data: bytes(&instruction.data),
                                                stack_height: instruction.stack_height,
                                            })
                                            .collect(),
                                    })
                                    .collect(),
                                pre_token_balances: meta
                                    .pre_token_balances
                                    .iter()
                                    .map(|balance| $crate::model::TokenBalance {
                                        account_index: balance.account_index,
                                        mint: &balance.mint,
                                        owner: &balance.owner,
                                        program_id: &balance.program_id,
                                    })
                                    .collect(),
                                post_token_balances: meta
                                    .post_token_balances
                                    .iter()
                                    .map(|balance| $crate::model::TokenBalance {
                                        account_index: balance.account_index,
                                        mint: &balance.mint,
                                        owner: &balance.owner,
                                        program_id: &balance.program_id,
                                    })
                                    .collect(),
                            }),
                        }
                    })
                    .collect(),
            }
        }
    };
}
