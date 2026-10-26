//! Captured public mainnet transactions, selected without changing their contents.
//! Source indices and conversion limits are documented in fixtures/README.md.
use buffa::Message;
use dex_pools::{solana::base58, Block, EvidencePosition, InstructionPosition, Pool, Protocol};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

fn fixture(compressed: &[u8], checksum: &str, slot: u64, signature: &str) -> Block {
    let mut bytes = Vec::new();
    GzDecoder::new(compressed).read_to_end(&mut bytes).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), checksum);
    let block = Block::decode_from_slice(&bytes).unwrap();
    assert_eq!(block.encode_to_vec(), bytes);
    assert_eq!(block.slot, slot);
    // These are explicitly selected transaction fixtures, not complete blocks.
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(base58::encode(block.transactions[0].hash()), signature);
    block
}

fn only_pool(block: &Block, protocol: Protocol, address: &str, instruction: InstructionPosition) -> Pool {
    let projection = dex_pools::project(block).unwrap();
    assert_eq!(projection.pools.len(), 1);
    let pool = projection.pools.into_iter().next().unwrap();
    assert_eq!(pool.protocol, protocol);
    assert_eq!(base58::encode(pool.address), address);
    assert_eq!(pool.creation.slot, block.slot);
    assert_eq!(pool.creation.blockhash, block.blockhash);
    assert_eq!(pool.creation.transaction_index, 0);
    assert_eq!(pool.creation.signature.as_slice(), block.transactions[0].hash());
    assert_eq!(pool.creation.instruction, instruction);
    pool
}

fn token(pool: &Pool, index: usize, mint: &str, vault: &str, decimals: u8, program: &str) {
    assert_eq!(base58::encode(pool.tokens[index].mint), mint);
    assert_eq!(base58::encode(pool.tokens[index].vault.unwrap()), vault);
    assert_eq!(pool.tokens[index].decimals, Some(decimals));
    assert_eq!(base58::encode(pool.tokens[index].token_program.unwrap()), program);
}

#[test]
fn captured_pump_v2_decodes_but_truncated_trace_is_rejected() {
    let block = fixture(
        include_bytes!("../fixtures/pump-v2-349999876-selected.pb.gz"),
        "3aecbc2513e47ac36a5ae394e7e496592cafd5229d777fcf5ad9773a0fc9e622",
        349999876,
        "5rg7xHWAG6jDohDMbHEboHw3z7tDpoXdiLWRMTQ9fcFEWKEMBUE9nuyJvssA6xA3ZfV8b1ppGPreC5UzXtBX5omX",
    );
    let tx = &block.transactions[0];
    assert!(tx.meta.log_messages.iter().any(|line| line == "Log truncated"));
    assert!(common::invocation::prove(tx).is_none());
    let instructions: std::collections::BTreeMap<_, _> = common::invocation::instruction_positions(tx).into_iter().zip(tx.walk_instructions()).collect();
    let init = &instructions[&InstructionPosition {
        instruction_index: 2,
        inner_instruction_index: Some(22),
    }];
    let event = &instructions[&InstructionPosition {
        instruction_index: 2,
        inner_instruction_index: Some(34),
    }];
    let accounts = init.accounts();
    let borrowed: Vec<&[u8]> = accounts.iter().map(|account| account.0.as_slice()).collect();
    // Historical V2 bytes still decode as a matching candidate. They cannot
    // enter the store because the whole transaction's trace was truncated.
    let candidate = dex_pools::decode_pool_creation(init.program_id().0, init.data(), &borrowed, event.data()).unwrap();
    assert_eq!(candidate.protocol, Protocol::PumpfunAmm);
    assert_eq!(base58::encode(candidate.address), "78NwZQYMgSZiCfCZvrNpjpRD34W4A3YVjMzWTmm87498");
    assert_eq!(base58::encode(candidate.tokens[0].mint), "7kjAN4oY9cJ4LDUha7oPdrfUGocqjFtMTTBcg98Wpump");
    assert_eq!(base58::encode(candidate.tokens[1].mint), "So11111111111111111111111111111111111111112");
    assert!(dex_pools::project(&block).unwrap().pools.is_empty());
}

#[test]
fn captured_current_pump_creation_supports_token_2022_and_non_sol_quote() {
    let block = fixture(
        include_bytes!("../fixtures/pump-current-461766718-selected.pb.gz"),
        "c75669f6f39b0dffb3cae60f18d8d47974ac08ef6a816075844fed68c54e885f",
        461766718,
        "4kEbHbpER2yoxyjFz5Gf9UxQTckXEFMcbqZTcRbVEXCZhJssSYXCpZrqfBFCpeFoQgw77w36zox7iKf2YtzU6uBb",
    );
    let pool = only_pool(
        &block,
        Protocol::PumpfunAmm,
        "HyWCWry19VtNwgjmkQ5r9tKZHZT9eJPQu9hW9fKQDBaN",
        InstructionPosition {
            instruction_index: 2,
            inner_instruction_index: Some(23),
        },
    );
    assert_eq!(
        pool.creation.evidence,
        EvidencePosition::Instruction(InstructionPosition {
            instruction_index: 2,
            inner_instruction_index: Some(35)
        })
    );
    token(
        &pool,
        0,
        "CyZ3khPnhAWmZNENcWN1VKwDCi3niJd5CCDa9NLrpump",
        "8VieFsChtHoMAfpE2t8SMGmwjj7EY4eKKAU8y2u85X1i",
        6,
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    );
    token(
        &pool,
        1,
        "pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn",
        "96mai37T3kQX296CGd7gEb9HVC6MiDQGXUnaCCdbXp8r",
        6,
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    );
}

#[test]
fn captured_orca_v2_creation_has_exact_accounts_and_log_provenance() {
    let block = fixture(
        include_bytes!("../fixtures/orca-v2-461667419-selected.pb.gz"),
        "dbd820bc1d8b17e15db38b02c642d38827018b5458c09326cdcf80ab800793ed",
        461667419,
        "3TkKYXzYcfsw7Bpr6tmXgEjnstHdQY5jpxzYmWQ6YosMyjnomN5wy9pCAfdh87THpaPEziirhaS1D8X9yPSmsUoc",
    );
    let pool = only_pool(
        &block,
        Protocol::OrcaWhirlpool,
        "2BRaLsFfxQ3WFcFE9izDPge5fcXTLYJwtauBYsTf5nEW",
        InstructionPosition {
            instruction_index: 2,
            inner_instruction_index: None,
        },
    );
    assert_eq!(pool.creation.evidence, EvidencePosition::Log { log_index: 26 });
    token(
        &pool,
        0,
        "EkAookfa7Js3VZYumT3NP6nHwEAUJfm8NKJ2jMCcrap",
        "7Sif74WScadBUA31mQXCdi9qGRJzNHNLuAjAnxZ1s2q4",
        6,
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    );
    token(
        &pool,
        1,
        "CARDSccUMFKoPRZxt5vt3ksUbxEFEcnZ3H2pd3dKxYjp",
        "FfehaE62kTaerPy9Xb4fGiDjYQBn4Kww584FHbZt3XXN",
        6,
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    );
}
