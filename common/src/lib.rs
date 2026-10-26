#[cfg(feature = "substreams")]
pub mod db;
pub mod invocation;
pub mod solana;
#[cfg(feature = "substreams")]
use substreams::{log, pb::substreams::Clock, scalar::BigInt};

pub type Address = Vec<u8>;
pub type Hash = Vec<u8>;
pub const NULL_ADDRESS: [u8; 20] = [0; 20];
pub const NULL_HASH: [u8; 32] = [0; 32];
pub const NATIVE_ADDRESS: [u8; 20] = [0xee; 20];

// In ClickHouse, an aggregate function like argMax can only take one expression as the "ordering" argument.
// So we typically combine (block_num, index) into a single monotonic integer.
// For example, if each of block_num and index fits in 32 bits, we can do:
// max(toUInt64(block_num) * 2^32 + index) AS version
#[cfg(feature = "substreams")]
pub fn to_global_sequence(clock: &Clock, index: u64) -> u64 {
    (clock.number << 32) + index
}

pub fn bytes32_to_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "".to_string();
    }
    let s = String::from_utf8_lossy(&bytes);
    s.trim().trim_matches('\0').to_string()
}

// Used to enforce ERC-20 decimals to be between 0 and 255
#[cfg(feature = "substreams")]
pub fn bigint_to_uint8(bigint: &substreams::scalar::BigInt) -> Option<i32> {
    if bigint.lt(&BigInt::zero()) {
        log::info!("bigint_to_uint8: value is negative");
        return None;
    }
    if bigint.gt(&BigInt::from(255)) {
        log::info!("bigint_to_uint8: value is greater than 255");
        return None;
    }
    Some(bigint.to_i32())
}

#[cfg(feature = "substreams")]
pub fn bigint_to_uint64(bigint: &substreams::scalar::BigInt) -> Option<u64> {
    if bigint.lt(&BigInt::zero()) {
        log::info!("bigint_to_uint64: value is negative");
        return None;
    }
    if bigint.gt(&BigInt::from(u64::MAX)) {
        log::info!("bigint_to_uint64: value is greater than u64::MAX");
        return None;
    }
    Some(bigint.to_u64())
}

#[cfg(feature = "substreams")]
pub fn bigint_to_i32(bigint: &substreams::scalar::BigInt) -> Option<i32> {
    if bigint.lt(&BigInt::zero()) {
        log::info!("bigint_to_i32: value is negative");
        return None;
    }
    if bigint.gt(&BigInt::from(i32::MAX)) {
        log::info!("bigint_to_i32: value is greater than i32::MAX");
        return None;
    }
    Some(bigint.to_i32())
}

pub fn is_zero_address<T: AsRef<[u8]>>(addr: T) -> bool {
    addr.as_ref() == NULL_ADDRESS
}
