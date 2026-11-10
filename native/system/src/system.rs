use proto::pb::solana::native::token::v1 as pb;
use solana_program::system_instruction::SystemInstruction;

use bincode::config;
use substreams_solana::block_view::InstructionView;

#[cfg(any(feature = "substreams", test))]
pub fn unpack_transfers(instruction: &InstructionView) -> Option<pb::instruction::Instruction> {
    try_unpack_transfers(instruction).ok().flatten()
}

/// `SystemInstruction::CreateAccountAllowPrefund` (SIMD-0312), which the pinned
/// `solana-program` enum does not have. Fixed-width bincode, like every System
/// instruction: this discriminant, then `lamports: u64`, `space: u64` and
/// `owner: Pubkey`.
const CREATE_ACCOUNT_ALLOW_PREFUND: u32 = 13;
const CREATE_ACCOUNT_ALLOW_PREFUND_LEN: usize = 4 + 8 + 8 + 32;

/// Decodes `CreateAccountAllowPrefund`, or returns `None` for any other
/// discriminant. Its accounts are the reverse of `CreateAccount`: the new
/// account first, then the funding account, which is present only when
/// `lamports > 0`. With lamports it moves them from the funding account to the
/// new account exactly as `CreateAccount` does, so it is reported as one.
fn try_unpack_create_account_allow_prefund(instruction: &InstructionView) -> Option<Result<Option<pb::instruction::Instruction>, ()>> {
    let data = instruction.data();
    if data.len() < 4 || u32::from_le_bytes(data[..4].try_into().ok()?) != CREATE_ACCOUNT_ALLOW_PREFUND {
        return None;
    }
    Some((|| {
        // Trailing bytes are ignored, as bincode ignores them for the other variants.
        let body = data.get(4..CREATE_ACCOUNT_ALLOW_PREFUND_LEN).ok_or(())?;
        let lamports = u64::from_le_bytes(body[..8].try_into().map_err(|_| ())?);
        let space = u64::from_le_bytes(body[8..16].try_into().map_err(|_| ())?);
        let owner = body[16..48].to_vec();
        if lamports == 0 {
            return Ok(None);
        }
        let accounts = instruction.accounts();
        let new_account = accounts.first().map(|value| value.0.to_vec()).ok_or(())?;
        let source = accounts.get(1).map(|value| value.0.to_vec()).ok_or(())?;
        Ok(Some(pb::instruction::Instruction::CreateAccount(
            pb::CreateAccount {
                source,
                new_account,
                lamports,
                space,
                owner,
            }
            .into(),
        )))
    })())
}

pub fn try_unpack_transfers(instruction: &InstructionView) -> Result<Option<pb::instruction::Instruction>, ()> {
    if let Some(decoded) = try_unpack_create_account_allow_prefund(instruction) {
        return decoded;
    }
    let cfg = config::standard()
        .with_limit::<4096>()
        .with_fixed_int_encoding() // NOT variable‑int
        .with_little_endian();

    let (sys_ix, _): (SystemInstruction, usize) = bincode::serde::decode_from_slice(instruction.data(), cfg).map_err(|_| ())?;
    let accounts = instruction.accounts();
    let account = |index: usize| accounts.get(index).map(|value| value.0.to_vec()).ok_or(());

    Ok(match sys_ix {
        SystemInstruction::Transfer { lamports } if lamports > 0 => {
            let source = account(0)?;
            let destination = account(1)?;

            Some(pb::instruction::Instruction::Transfer(pb::Transfer { source, destination, lamports }.into()))
        }
        SystemInstruction::TransferWithSeed {
            lamports,
            from_owner,
            from_seed,
        } if lamports > 0 => {
            let source = account(0)?;
            let source_base = account(1)?;
            let destination = account(2)?;

            Some(pb::instruction::Instruction::TransferWithSeed(
                pb::TransferWithSeed {
                    destination,
                    lamports,
                    source,
                    source_owner: from_owner.to_bytes().to_vec(),
                    source_base,
                    source_seed: from_seed,
                }
                .into(),
            ))
        }
        SystemInstruction::CreateAccount { space, owner, lamports } if lamports > 0 => {
            let source = account(0)?;
            let new_account = account(1)?;

            Some(pb::instruction::Instruction::CreateAccount(
                pb::CreateAccount {
                    source,
                    new_account,
                    lamports,
                    space,
                    owner: owner.to_bytes().to_vec(),
                }
                .into(),
            ))
        }
        SystemInstruction::CreateAccountWithSeed {
            base,
            seed,
            space,
            owner,
            lamports,
        } if lamports > 0 => {
            let source = account(0)?;
            let new_account = account(1)?;
            let base_account = accounts.get(2).map(|account| account.0.to_vec());

            Some(pb::instruction::Instruction::CreateAccountWithSeed(
                pb::CreateAccountWithSeed {
                    source,
                    new_account,
                    lamports,
                    space,
                    owner: owner.to_bytes().to_vec(),
                    base_account,
                    base: base.to_bytes().to_vec(),
                    seed,
                }
                .into(),
            ))
        }
        SystemInstruction::WithdrawNonceAccount { 0: lamports } if lamports > 0 => {
            let nonce_account = account(0)?;
            let destination = account(1)?;
            // If nonce_authority isn't specified (at index 4), use the nonce_account as the authority
            let nonce_authority = accounts.get(4).map_or(nonce_account.clone(), |account| account.0.to_vec());

            Some(pb::instruction::Instruction::WithdrawNonceAccount(
                pb::WithdrawNonceAccount {
                    nonce_account,
                    destination,
                    lamports,
                    nonce_authority,
                }
                .into(),
            ))
        }
        _ => None,
    })
}
