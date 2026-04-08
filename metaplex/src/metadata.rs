use borsh::BorshDeserialize;
use mpl_token_metadata::instructions::{
    CreateMetadataAccountV3InstructionArgs, CreateV1InstructionArgs, UpdateMetadataAccountV2InstructionArgs, UpdateV1InstructionArgs,
};
use mpl_token_metadata::types::{Data, DataV2};
use proto::pb::solana::metaplex::v1 as pb;
use substreams_solana::block_view::InstructionView;

use crate::is_metaplex_program;

/// Strip null bytes and trailing whitespace from Metaplex fixed-length strings.
fn trim_null(s: &str) -> String {
    s.trim_end_matches('\0').trim().to_string()
}

fn create_metadata_account(
    instruction: &InstructionView,
    metadata_index: usize,
    mint_index: usize,
    mint_authority_index: usize,
    payer_index: usize,
    update_authority_index: usize,
    name: &str,
    symbol: &str,
    uri: &str,
) -> Option<pb::instruction::Instruction> {
    Some(pb::instruction::Instruction::CreateMetadataAccount(pb::CreateMetadataAccount {
        metadata: instruction.accounts().get(metadata_index)?.0.to_vec(),
        mint: instruction.accounts().get(mint_index)?.0.to_vec(),
        mint_authority: instruction.accounts().get(mint_authority_index)?.0.to_vec(),
        payer: instruction.accounts().get(payer_index)?.0.to_vec(),
        update_authority: instruction.accounts().get(update_authority_index)?.0.to_vec(),
        name: trim_null(name),
        symbol: trim_null(symbol),
        uri: trim_null(uri),
    }))
}

fn update_metadata_account(
    instruction: &InstructionView,
    metadata_index: usize,
    update_authority_index: usize,
    name: Option<String>,
    symbol: Option<String>,
    uri: Option<String>,
) -> Option<pb::instruction::Instruction> {
    Some(pb::instruction::Instruction::UpdateMetadataAccount(pb::UpdateMetadataAccount {
        metadata: instruction.accounts().get(metadata_index)?.0.to_vec(),
        update_authority: instruction.accounts().get(update_authority_index)?.0.to_vec(),
        name,
        symbol,
        uri,
    }))
}

pub fn unpack_metadata(instruction: &InstructionView, program_id: &[u8]) -> Option<pb::instruction::Instruction> {
    if !is_metaplex_program(program_id) {
        return None;
    }

    let data = instruction.data();
    let (discriminator, mut rest) = data.split_first()?;

    match discriminator {
        0 => {
            #[derive(BorshDeserialize)]
            struct Args {
                data: Data,
                // is_mutable: bool,
            }
            let args: Args = Args::deserialize(&mut rest).ok()?;
            create_metadata_account(instruction, 0, 1, 2, 3, 4, &args.data.name, &args.data.symbol, &args.data.uri)
        }
        16 => {
            #[derive(BorshDeserialize)]
            struct Args {
                data: DataV2,
                // is_mutable: bool,
            }
            let args: Args = Args::deserialize(&mut rest).ok()?;
            create_metadata_account(instruction, 0, 1, 2, 3, 4, &args.data.name, &args.data.symbol, &args.data.uri)
        }
        33 => {
            let args = CreateMetadataAccountV3InstructionArgs::deserialize(&mut rest).ok()?;
            let data = args.data;
            create_metadata_account(instruction, 0, 1, 2, 3, 4, &data.name, &data.symbol, &data.uri)
        }
        42 => {
            let args = CreateV1InstructionArgs::deserialize(&mut rest).ok()?;
            create_metadata_account(instruction, 0, 2, 3, 4, 5, &args.name, &args.symbol, &args.uri)
        }
        1 => {
            #[derive(BorshDeserialize)]
            struct Args {
                data: Option<Data>,
                // update_authority: Option<[u8; 32]>,
                // primary_sale_happened: Option<bool>,
            }
            let args: Args = Args::deserialize(&mut rest).ok()?;
            let (name, symbol, uri) = if let Some(data) = args.data {
                (Some(trim_null(&data.name)), Some(trim_null(&data.symbol)), Some(trim_null(&data.uri)))
            } else {
                (None, None, None)
            };
            update_metadata_account(instruction, 0, 1, name, symbol, uri)
        }
        15 => {
            let args = UpdateMetadataAccountV2InstructionArgs::deserialize(&mut rest).ok()?;
            let (name, symbol, uri) = if let Some(data) = args.data {
                (Some(trim_null(&data.name)), Some(trim_null(&data.symbol)), Some(trim_null(&data.uri)))
            } else {
                (None, None, None)
            };
            update_metadata_account(instruction, 0, 1, name, symbol, uri)
        }
        50 => {
            let args = UpdateV1InstructionArgs::deserialize(&mut rest).ok()?;
            let (name, symbol, uri) = if let Some(data) = args.data {
                (Some(trim_null(&data.name)), Some(trim_null(&data.symbol)), Some(trim_null(&data.uri)))
            } else {
                (None, None, None)
            };
            update_metadata_account(instruction, 4, 0, name, symbol, uri)
        }
        _ => None,
    }
}
