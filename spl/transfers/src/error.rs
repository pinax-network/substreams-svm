use crate::decode::DecodeError;
use std::fmt;

/// A message shape outside the projector's legacy/v0/v1 coverage, discovered by
/// the Buffa adapter before newer schema fields would be lost in the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedMessage {
    /// Original transaction position, including votes and failures.
    pub transaction_index: usize,
    pub version: Option<u32>,
    pub versioned: bool,
    pub inline_configuration: bool,
    /// Number of address table lookups; a v1 message must have none.
    pub address_table_lookups: usize,
}

impl fmt::Display for UnsupportedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "transaction {}: unsupported message version {:?} (versioned: {}, inline configuration: {}, address table lookups: {})",
            self.transaction_index, self.version, self.versioned, self.inline_configuration, self.address_table_lookups
        )
    }
}

impl std::error::Error for UnsupportedMessage {}

/// Original position of an instruction inside its transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    /// Index of the outer (top-level) instruction in the message.
    pub instruction_index: u32,
    /// Index inside the inner instructions recorded for `instruction_index`;
    /// `None` for the outer instruction itself.
    pub inner_instruction_index: Option<u32>,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner_instruction_index {
            None => write!(f, "instruction {}", self.instruction_index),
            Some(inner) => write!(f, "instruction {}.{}", self.instruction_index, inner),
        }
    }
}

/// Why a block was rejected. Malformed input is an error, never a panic or a
/// silently misattributed account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectError {
    /// `blockhash` or `previous_blockhash` is not a base58 32-byte hash.
    InvalidBlockHash { field: &'static str, value: String },
    /// The parent slot is not below the slot (skipped slots are allowed).
    InvalidParentSlot { slot: u64, parent_slot: u64 },
    /// The block time is negative.
    NegativeBlockTime { block_time: i64 },
    /// Transaction at `transaction_index` (original block position) is malformed.
    Transaction { transaction_index: u32, error: TransactionError },
}

/// A malformed transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionError {
    MissingMessage,
    MissingHeader,
    MissingMeta,
    /// A signature is not 64 bytes; the first signature identifies the transaction.
    InvalidSignature {
        position: usize,
        len: usize,
    },
    /// Signature count inconsistent with the header and account keys.
    SignatureCount {
        signatures: usize,
        required: u32,
        account_keys: usize,
    },
    /// A static or loaded key is not 32 bytes; `position` is in the resolved key table.
    InvalidAccountKey {
        position: usize,
        len: usize,
    },
    /// More resolved keys than one-byte instruction indexes can address.
    TooManyAccountKeys {
        count: usize,
    },
    /// Loaded address counts differ from the address table lookups.
    LoadedAddressCount {
        writable: usize,
        readonly: usize,
        lookup_writable: usize,
        lookup_readonly: usize,
    },
    ProgramIndexOutOfBounds {
        position: Position,
        program_id_index: u32,
        key_count: usize,
    },
    AccountIndexOutOfBounds {
        position: Position,
        account_position: usize,
        account_index: u8,
        key_count: usize,
    },
    /// Inner instructions are recorded under a missing outer instruction.
    InnerInstructionsOutOfBounds {
        index: u32,
        instruction_count: usize,
    },
    /// Inner instructions are recorded twice for one outer instruction.
    DuplicateInnerInstructions {
        index: u32,
    },
    TokenBalanceAccountOutOfBounds {
        account_index: u32,
        key_count: usize,
    },
    /// A token balance owner is neither empty nor a base58 32-byte key.
    InvalidTokenBalanceOwner {
        account_index: u32,
    },
    /// An amount-moving token instruction of a successful transaction does not
    /// fit its layout, which the token programs would have rejected.
    Decode {
        position: Position,
        error: DecodeError,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBlockHash { field, value } => write!(f, "invalid block {field} {value:?}"),
            Self::InvalidParentSlot { slot, parent_slot } => write!(f, "parent slot {parent_slot} is not below slot {slot}"),
            Self::NegativeBlockTime { block_time } => write!(f, "negative block time {block_time}"),
            Self::Transaction { transaction_index, error } => write!(f, "transaction {transaction_index}: {error}"),
        }
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMessage => f.write_str("missing message"),
            Self::MissingHeader => f.write_str("missing message header"),
            Self::MissingMeta => f.write_str("missing status meta"),
            Self::InvalidSignature { position, len } => write!(f, "signature {position} has {len} bytes"),
            Self::SignatureCount {
                signatures,
                required,
                account_keys,
            } => write!(f, "{signatures} signatures for {required} required signatures and {account_keys} account keys"),
            Self::InvalidAccountKey { position, len } => write!(f, "account key {position} has {len} bytes"),
            Self::TooManyAccountKeys { count } => write!(f, "{count} account keys"),
            Self::LoadedAddressCount {
                writable,
                readonly,
                lookup_writable,
                lookup_readonly,
            } => write!(
                f,
                "{writable} writable and {readonly} readonly loaded addresses for lookups of {lookup_writable} and {lookup_readonly}"
            ),
            Self::ProgramIndexOutOfBounds {
                position,
                program_id_index,
                key_count,
            } => write!(f, "{position}: program index {program_id_index} out of {key_count} keys"),
            Self::AccountIndexOutOfBounds {
                position,
                account_position,
                account_index,
                key_count,
            } => write!(f, "{position}: account {account_position} index {account_index} out of {key_count} keys"),
            Self::InnerInstructionsOutOfBounds { index, instruction_count } => {
                write!(f, "inner instructions for instruction {index} of {instruction_count}")
            }
            Self::DuplicateInnerInstructions { index } => write!(f, "inner instructions recorded twice for instruction {index}"),
            Self::TokenBalanceAccountOutOfBounds { account_index, key_count } => {
                write!(f, "token balance account index {account_index} out of {key_count} keys")
            }
            Self::InvalidTokenBalanceOwner { account_index } => write!(f, "invalid token balance owner for account index {account_index}"),
            Self::Decode { position, error } => write!(f, "{position}: {error}"),
        }
    }
}

impl std::error::Error for ProjectError {}
impl std::error::Error for TransactionError {}
