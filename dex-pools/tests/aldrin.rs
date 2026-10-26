//! Synthetic fixtures use the official V2 registry's named account order:
//! aldrin-labs/aldrin-registry@b1db26ca326356e7bbdd582985a12f6c7a2bbdcf,
//! src/idls/poolsV2.json. Shared-IDL provenance binds that model to CURVGo...
//! through @aldrin_exchange/sdk 0.4.51. These are not mainnet captures.
use dex_pools::{
    decode_pool_initialization, decode_pool_initialization_with_token_accounts, project,
    solana::{base58, pb::sf::solana::r#type::v1 as pb},
    EvidencePosition, Protocol, TokenAccountMetadata,
};
use substreams_solana_idls::{aldrin, spl::token};

fn initializer() -> Vec<u8> {
    let mut data = aldrin::instructions::INITIALIZE.to_vec();
    data.extend(
        borsh::to_vec(&aldrin::instructions::InitializeInstruction {
            signer_nonce: 254,
            curve_type: 1,
        })
        .unwrap(),
    );
    data
}

fn accounts() -> Vec<[u8; 32]> {
    // pool, poolMint, lpTokenFreezeVault, baseTokenVault, quoteTokenVault,
    // poolSigner, initializer, poolAuthority, feeBaseAccount, feeQuoteAccount,
    // feePoolTokenAccount, curve, tokenProgram, rent.
    let mut accounts: Vec<_> = (10..24).map(|index| [index; 32]).collect();
    accounts[12] = token::PROGRAM_ID;
    accounts
}

fn metadata() -> Vec<TokenAccountMetadata> {
    vec![
        TokenAccountMetadata {
            address: [13; 32],
            mint: [90; 32],
            decimals: Some(0),
            token_program: Some(token::PROGRAM_ID),
        },
        TokenAccountMetadata {
            address: [14; 32],
            mint: [40; 32],
            decimals: Some(9),
            token_program: None,
        },
    ]
}

fn block() -> pb::Block {
    let accounts = accounts();
    let mut keys = vec![vec![240; 32], aldrin::PROGRAM_ID.to_vec()];
    keys.extend(accounts.iter().map(|key| key.to_vec()));
    let balance = |account_index, mint, decimals| pb::TokenBalance {
        account_index,
        mint: base58::encode([mint; 32]),
        program_id: base58::encode(token::PROGRAM_ID),
        ui_token_amount: pb::UiTokenAmount {
            decimals,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    pb::Block {
        slot: 123,
        parent_slot: 120,
        blockhash: base58::encode([200; 32]),
        previous_blockhash: base58::encode([199; 32]),
        transactions: vec![pb::ConfirmedTransaction {
            transaction: pb::Transaction {
                signatures: vec![vec![7; 64]],
                message: pb::Message {
                    header: pb::MessageHeader {
                        num_required_signatures: 1,
                        ..Default::default()
                    }
                    .into(),
                    account_keys: keys,
                    recent_blockhash: vec![9; 32],
                    instructions: vec![pb::CompiledInstruction {
                        program_id_index: 1,
                        accounts: (2..accounts.len() as u8 + 2).collect(),
                        data: initializer(),
                    }],
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            }
            .into(),
            meta: pb::TransactionStatusMeta {
                post_token_balances: vec![balance(5, 90, 0), balance(6, 40, 9)],
                log_messages: vec![
                    format!("Program {} invoke [1]", base58::encode(aldrin::PROGRAM_ID)),
                    format!("Program {} success", base58::encode(aldrin::PROGRAM_ID)),
                ],
                ..Default::default()
            }
            .into(),
        }],
        ..Default::default()
    }
}

#[test]
fn v2_initializer_requires_vault_context_and_preserves_base_quote_order() {
    let accounts = accounts();
    let refs: Vec<_> = accounts.iter().map(|key| key.as_slice()).collect();
    let data = initializer();
    assert!(decode_pool_initialization(&aldrin::PROGRAM_ID, &data, &refs).is_none());
    let pool = decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &data, &refs, &metadata()).unwrap();
    assert_eq!(pool.protocol, Protocol::Aldrin);
    assert_eq!(pool.address, [10; 32]);
    assert_eq!(pool.tokens.iter().map(|token| token.mint).collect::<Vec<_>>(), vec![[90; 32], [40; 32]]);
    assert_eq!(pool.tokens[0].vault, Some([13; 32]));
    assert_eq!(pool.tokens[1].vault, Some([14; 32]));
    assert_eq!(pool.tokens[0].decimals, Some(0));
    assert!(pool
        .tokens
        .iter()
        .all(|entry| entry.token_program == Some(token::PROGRAM_ID) && !entry.is_native));
}

#[test]
fn rejects_truncation_wrong_program_and_conflicting_vault_identity() {
    let accounts = accounts();
    let refs: Vec<_> = accounts.iter().map(|key| key.as_slice()).collect();
    let data = initializer();
    for end in 0..data.len() {
        assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &data[..end], &refs, &metadata()).is_none());
    }
    let mut extended = data.clone();
    extended.push(0);
    assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &extended, &refs, &metadata()).is_none());
    for end in 0..refs.len() {
        assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &data, &refs[..end], &metadata()).is_none());
    }
    assert!(decode_pool_initialization_with_token_accounts(&[42; 32], &data, &refs, &metadata()).is_none());
    let mut wrong_vault = metadata();
    wrong_vault[0].address = [11; 32]; // The LP mint cannot substitute for the base vault.
    assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &data, &refs, &wrong_vault).is_none());
    let mut wrong_program = metadata();
    wrong_program[0].token_program = Some([33; 32]);
    assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &data, &refs, &wrong_program).is_none());
    let mut conflict = metadata();
    let mut changed = conflict[0].clone();
    changed.mint = [88; 32];
    conflict.push(changed);
    assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &data, &refs, &conflict).is_none());
    // The pre-existing shared CLOB API is not an AMM V2 pool constructor.
    let mut clob = aldrin::instructions::INITIALIZE_MARKET.to_vec();
    clob.extend(
        borsh::to_vec(&aldrin::instructions::InitializeMarketInstruction {
            coin_lot_size: 1,
            pc_lot_size: 1,
            fee_rate_bps: 1,
            vault_signer_nonce: 1,
            pc_dust_threshold: 1,
        })
        .unwrap(),
    );
    assert!(decode_pool_initialization_with_token_accounts(&aldrin::PROGRAM_ID, &clob, &refs, &metadata()).is_none());
}

#[test]
fn projection_uses_exact_vault_metadata_and_requires_complete_success() {
    let input = block();
    let output = project(&input).unwrap();
    assert_eq!(output.pools.len(), 1);
    assert_eq!(output.pools[0].protocol, Protocol::Aldrin);
    assert_eq!(output.pools[0].creation.evidence, EvidencePosition::Initialization);
    assert_eq!(output.pools[0].tokens[0].mint, [90; 32]);
    let mut missing = input.clone();
    missing.transactions[0].meta.as_option_mut().unwrap().post_token_balances.pop();
    assert!(project(&missing).unwrap().pools.is_empty());
    let mut conflicting = input.clone();
    let meta = conflicting.transactions[0].meta.as_option_mut().unwrap();
    let mut previous = meta.post_token_balances[0].clone();
    previous.mint = base58::encode([89; 32]);
    meta.pre_token_balances.push(previous);
    assert!(project(&conflicting).unwrap().pools.is_empty());
    let mut unclosed = input.clone();
    unclosed.transactions[0].meta.as_option_mut().unwrap().log_messages.pop();
    assert!(project(&unclosed).unwrap().pools.is_empty());
    let mut failed = input.clone();
    failed.transactions[0].meta.as_option_mut().unwrap().err = pb::TransactionError { err: vec![1] }.into();
    assert!(project(&failed).unwrap().pools.is_empty());
    let mut truncated = input;
    truncated.transactions[0]
        .meta
        .as_option_mut()
        .unwrap()
        .log_messages
        .push("Log truncated".into());
    assert!(project(&truncated).unwrap().pools.is_empty());
}

#[test]
fn captured_mainnet_v2_initialization_has_exact_context_and_provenance() {
    use buffa::Message;
    use flate2::read::GzDecoder;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../fixtures/aldrin-v2-193097357-selected.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "74be9ca3a061c137f8dd90374198a93d2bd522da97f2fec36a5717d8396facb6"
    );
    let block = pb::Block::decode_from_slice(&bytes).unwrap();
    assert_eq!(block.encode_to_vec(), bytes);
    assert_eq!(block.slot, 193097357);
    assert_eq!(block.transactions.len(), 1); // Selected transaction; original block index 143.
    assert_eq!(
        base58::encode(block.transactions[0].hash()),
        "4C1z3zxydcyVKGJFN8ahpwBGMfjmQGYHrMMoFzi768ue1eVYeGDbomcSSYVV4RdRWmKyb5bTyqZ4CK9ymjBxA1GJ"
    );
    let output = project(&block).unwrap();
    assert_eq!(output.pools.len(), 1);
    let pool = &output.pools[0];
    assert_eq!(pool.protocol, Protocol::Aldrin);
    assert_eq!(base58::encode(pool.address), "931bAjQx4RtbBjreP8HTNEu7ykozk4QXyYj3hiaJNdH");
    assert_eq!(
        pool.creation.instruction,
        dex_pools::InstructionPosition {
            instruction_index: 2,
            inner_instruction_index: None
        }
    );
    assert_eq!(pool.creation.evidence, EvidencePosition::Initialization);
    assert_eq!(pool.creation.transaction_index, 0);
    assert_eq!(pool.creation.signature.as_slice(), block.transactions[0].hash());
    assert_eq!(pool.creation.blockhash, block.blockhash);
    assert_eq!(pool.tokens.len(), 2);
    for (token, mint, vault, decimals) in [
        (
            &pool.tokens[0],
            "8uDiwgVP3BbFdvagB9nmuPyeB3YKnyWKWrqp16sWnFTC",
            "GBN31V2iuAf4vVszJopaeWtncQpaR6yCqiQZxrKxQNNc",
            9,
        ),
        (
            &pool.tokens[1],
            "FVUgGobxyaGT9fARpeGwL3VVxtjB8CfrCJz7UVQ3AEid",
            "A24aR7uX7bZmyTQqHoADthNdc9nCru25oc7uafw18EWk",
            0,
        ),
    ] {
        assert_eq!(base58::encode(token.mint), mint);
        assert_eq!(base58::encode(token.vault.unwrap()), vault);
        assert_eq!(token.decimals, Some(decimals));
        assert_eq!(token.token_program, Some(substreams_solana_idls::spl::token::PROGRAM_ID));
        assert!(!token.is_native);
    }
}
