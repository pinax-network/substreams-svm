use proto::pb::solana::metaplex::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::metaplex::token_metadata::instructions::{unpack, TokenMetadataInstruction};

/// Strip null bytes and trailing whitespace from Metaplex fixed-length strings.
fn trim_null(s: &str) -> String {
    s.trim_end_matches('\0').trim().to_string()
}

pub fn unpack_metadata(instruction: &InstructionView) -> Option<pb::instruction::Instruction> {
    let data = instruction.data();

    match unpack(data) {
        // CreateMetadataAccount V1 (disc 0)
        Ok(TokenMetadataInstruction::CreateMetadataAccount(args)) => {
            Some(pb::instruction::Instruction::CreateMetadataAccount(pb::CreateMetadataAccount {
                metadata: instruction.accounts().get(0)?.0.to_vec(),
                mint: instruction.accounts().get(1)?.0.to_vec(),
                mint_authority: instruction.accounts().get(2)?.0.to_vec(),
                payer: instruction.accounts().get(3)?.0.to_vec(),
                update_authority: instruction.accounts().get(4)?.0.to_vec(),
                name: trim_null(&args.data.name),
                symbol: trim_null(&args.data.symbol),
                uri: trim_null(&args.data.uri),
            }))
        }
        // CreateMetadataAccountV2 (disc 16)
        Ok(TokenMetadataInstruction::CreateMetadataAccountV2(args)) => {
            Some(pb::instruction::Instruction::CreateMetadataAccount(pb::CreateMetadataAccount {
                metadata: instruction.accounts().get(0)?.0.to_vec(),
                mint: instruction.accounts().get(1)?.0.to_vec(),
                mint_authority: instruction.accounts().get(2)?.0.to_vec(),
                payer: instruction.accounts().get(3)?.0.to_vec(),
                update_authority: instruction.accounts().get(4)?.0.to_vec(),
                name: trim_null(&args.data.name),
                symbol: trim_null(&args.data.symbol),
                uri: trim_null(&args.data.uri),
            }))
        }
        // CreateMetadataAccountV3 (disc 33)
        Ok(TokenMetadataInstruction::CreateMetadataAccountV3(args)) => {
            Some(pb::instruction::Instruction::CreateMetadataAccount(pb::CreateMetadataAccount {
                metadata: instruction.accounts().get(0)?.0.to_vec(),
                mint: instruction.accounts().get(1)?.0.to_vec(),
                mint_authority: instruction.accounts().get(2)?.0.to_vec(),
                payer: instruction.accounts().get(3)?.0.to_vec(),
                update_authority: instruction.accounts().get(4)?.0.to_vec(),
                name: trim_null(&args.data.name),
                symbol: trim_null(&args.data.symbol),
                uri: trim_null(&args.data.uri),
            }))
        }
        // UpdateMetadataAccount V1 (disc 1)
        Ok(TokenMetadataInstruction::UpdateMetadataAccount(args)) => {
            let (name, symbol, uri) = if let Some(data) = args.data {
                (Some(trim_null(&data.name)), Some(trim_null(&data.symbol)), Some(trim_null(&data.uri)))
            } else {
                (None, None, None)
            };
            Some(pb::instruction::Instruction::UpdateMetadataAccount(pb::UpdateMetadataAccount {
                metadata: instruction.accounts().get(0)?.0.to_vec(),
                update_authority: instruction.accounts().get(1)?.0.to_vec(),
                name,
                symbol,
                uri,
            }))
        }
        // UpdateMetadataAccountV2 (disc 15)
        Ok(TokenMetadataInstruction::UpdateMetadataAccountV2(args)) => {
            let (name, symbol, uri) = if let Some(data) = args.data {
                (Some(trim_null(&data.name)), Some(trim_null(&data.symbol)), Some(trim_null(&data.uri)))
            } else {
                (None, None, None)
            };
            Some(pb::instruction::Instruction::UpdateMetadataAccount(pb::UpdateMetadataAccount {
                metadata: instruction.accounts().get(0)?.0.to_vec(),
                update_authority: instruction.accounts().get(1)?.0.to_vec(),
                name,
                symbol,
                uri,
            }))
        }
        // CreateV1 (disc 42) — unified v1.13+ instruction
        // Accounts: metadata[0], master_edition[1], mint[2], authority[3], payer[4], update_authority[5]
        Ok(TokenMetadataInstruction::Create(args)) => {
            Some(pb::instruction::Instruction::CreateMetadataAccount(pb::CreateMetadataAccount {
                metadata: instruction.accounts().get(0)?.0.to_vec(),
                mint: instruction.accounts().get(2)?.0.to_vec(),
                mint_authority: instruction.accounts().get(3)?.0.to_vec(),
                payer: instruction.accounts().get(4)?.0.to_vec(),
                update_authority: instruction.accounts().get(5)?.0.to_vec(),
                name: trim_null(&args.name),
                symbol: trim_null(&args.symbol),
                uri: trim_null(&args.uri),
            }))
        }
        // UpdateV1 (disc 50) — unified v1.13+ instruction
        // Accounts: authority[0], delegate_record[1], token[2], mint[3], metadata[4]
        Ok(TokenMetadataInstruction::Update(args)) => {
            let (name, symbol, uri) = if let Some(data) = args.data {
                (Some(trim_null(&data.name)), Some(trim_null(&data.symbol)), Some(trim_null(&data.uri)))
            } else {
                (None, None, None)
            };
            Some(pb::instruction::Instruction::UpdateMetadataAccount(pb::UpdateMetadataAccount {
                metadata: instruction.accounts().get(4)?.0.to_vec(),
                update_authority: instruction.accounts().get(0)?.0.to_vec(),
                name,
                symbol,
                uri,
            }))
        }
        _ => None,
    }
}
