use dex_pools::{
    decode_pool_initialization, decode_pool_initialization_with_token_accounts, project,
    solana::{base58, pb::sf::solana::r#type::v1 as pb},
    EvidencePosition, Protocol, TokenAccountMetadata,
};
use substreams_solana_idls::{
    saros,
    spl::{token, token_swap},
    stabble,
};

fn initializer() -> Vec<u8> {
    let mut data = vec![token_swap::instructions::INITIALIZE];
    data.extend(
        borsh::to_vec(&token_swap::instructions::InitializeInstruction {
            fees: token_swap::accounts::Fees {
                trade_fee_numerator: 25,
                trade_fee_denominator: 10_000,
                owner_trade_fee_numerator: 5,
                owner_trade_fee_denominator: 10_000,
                owner_withdraw_fee_numerator: 0,
                owner_withdraw_fee_denominator: 1,
                host_fee_numerator: 20,
                host_fee_denominator: 100,
            },
            curve_type: 0,
            curve_parameters: [0; 32],
        })
        .unwrap(),
    );
    data
}
fn accounts() -> Vec<[u8; 32]> {
    let mut keys: Vec<_> = (10..18).map(|i| [i; 32]).collect();
    keys[7] = token::PROGRAM_ID;
    keys
}
fn metadata() -> Vec<TokenAccountMetadata> {
    vec![
        TokenAccountMetadata {
            address: [12; 32],
            mint: [30; 32],
            decimals: Some(0),
            token_program: Some(token::PROGRAM_ID),
        },
        TokenAccountMetadata {
            address: [13; 32],
            mint: [31; 32],
            decimals: Some(9),
            token_program: Some(token::PROGRAM_ID),
        },
    ]
}
fn block(program: [u8; 32], data: Vec<u8>, accounts: &[[u8; 32]]) -> pb::Block {
    let mut keys = vec![vec![240; 32], program.to_vec()];
    keys.extend(accounts.iter().map(|key| key.to_vec()));
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
                        data,
                    }],
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            }
            .into(),
            meta: pb::TransactionStatusMeta {
                log_messages: vec![
                    format!("Program {} invoke [1]", base58::encode(program)),
                    format!("Program {} success", base58::encode(program)),
                ],
                ..Default::default()
            }
            .into(),
        }],
        ..Default::default()
    }
}

#[test]
fn swap_initializers_require_exact_vault_context_and_preserve_zero_decimals() {
    let accounts = accounts();
    let refs: Vec<_> = accounts.iter().map(|key| key.as_slice()).collect();
    let data = initializer();
    for (program, protocol) in [(token_swap::PROGRAM_ID, Protocol::SplTokenSwap), (saros::PROGRAM_ID, Protocol::Saros)] {
        assert!(
            decode_pool_initialization(&program, &data, &refs).is_none(),
            "mints cannot be inferred from vault addresses"
        );
        let decoded = decode_pool_initialization_with_token_accounts(&program, &data, &refs, &metadata()).unwrap();
        assert_eq!(decoded.protocol, protocol);
        assert_eq!(decoded.address, [10; 32]);
        assert_eq!(decoded.tokens.iter().map(|t| t.mint).collect::<Vec<_>>(), vec![[30; 32], [31; 32]]);
        assert_eq!(decoded.tokens[0].vault, Some([12; 32]));
        assert_eq!(decoded.tokens[0].decimals, Some(0));
        let mut wrong = metadata();
        wrong[0].address = [250; 32];
        assert!(decode_pool_initialization_with_token_accounts(&program, &data, &refs, &wrong).is_none());
        let mut conflict = metadata();
        let mut extra = conflict[0].clone();
        extra.mint = [99; 32];
        conflict.push(extra);
        assert!(decode_pool_initialization_with_token_accounts(&program, &data, &refs, &conflict).is_none());
        let mut program_conflict = metadata();
        program_conflict[1].token_program = Some([77; 32]);
        assert!(decode_pool_initialization_with_token_accounts(&program, &data, &refs, &program_conflict).is_none());
        for end in 0..data.len() {
            assert!(decode_pool_initialization_with_token_accounts(&program, &data[..end], &refs, &metadata()).is_none());
        }
        assert!(decode_pool_initialization_with_token_accounts(&program, &data, &refs, &vec![metadata()[0].clone(); 513]).is_none());
    }
}

#[test]
fn complete_block_resolves_vaults_and_rejects_conflicting_or_unproved_context() {
    for program in [token_swap::PROGRAM_ID, saros::PROGRAM_ID] {
        let mut input = block(program, initializer(), &accounts());
        let balance = |index, mint, decimals| pb::TokenBalance {
            account_index: index,
            mint: base58::encode([mint; 32]),
            program_id: base58::encode(token::PROGRAM_ID),
            ui_token_amount: pb::UiTokenAmount {
                decimals,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        input.transactions[0].meta.as_option_mut().unwrap().post_token_balances = vec![balance(4, 30, 0), balance(5, 31, 9)];
        let result = project(&input).unwrap();
        assert_eq!(result.pools.len(), 1);
        assert_eq!(result.pools[0].creation.evidence, EvidencePosition::Initialization);
        let mut conflict = input.clone();
        conflict.transactions[0].meta.as_option_mut().unwrap().pre_token_balances = vec![balance(4, 32, 0)];
        assert!(project(&conflict).unwrap().pools.is_empty());
        let mut malformed = input.clone();
        malformed.transactions[0].meta.as_option_mut().unwrap().pre_token_balances = vec![balance(4, 30, 0)];
        malformed.transactions[0].meta.as_option_mut().unwrap().post_token_balances[0].mint = "invalid".into();
        assert!(project(&malformed).unwrap().pools.is_empty());
        input.transactions[0].meta.as_option_mut().unwrap().log_messages.pop();
        assert!(project(&input).unwrap().pools.is_empty());
    }
}

#[test]
fn weighted_pool_preserves_all_three_underlying_mints_and_excludes_lp_mint() {
    let accounts: Vec<_> = (50..59).map(|i| [i; 32]).collect();
    let refs: Vec<_> = accounts.iter().map(|key| key.as_slice()).collect();
    let mut data = stabble::instructions::INITIALIZE.to_vec();
    data.extend(
        borsh::to_vec(&stabble::instructions::InitializeInstruction {
            swap_fee: 100,
            weights: vec![3000, 3000, 4000],
            max_caps: vec![10000; 3],
        })
        .unwrap(),
    );
    let decoded = decode_pool_initialization(&stabble::PROGRAM_ID, &data, &refs).unwrap();
    assert_eq!(decoded.protocol, Protocol::Stabble);
    assert_eq!(decoded.address, [52; 32]);
    assert_eq!(decoded.tokens.iter().map(|t| t.mint).collect::<Vec<_>>(), vec![[56; 32], [57; 32], [58; 32]]);
    assert!(decoded
        .tokens
        .iter()
        .all(|t| t.vault.is_none() && t.decimals.is_none() && t.token_program.is_none() && !t.is_native));
    let output = project(&block(stabble::PROGRAM_ID, data.clone(), &accounts)).unwrap();
    assert_eq!(output.pools[0].tokens, decoded.tokens);
    assert!(decode_pool_initialization(&stabble::PROGRAM_ID, &data, &refs[..8]).is_none());
    let mut duplicates = accounts.clone();
    duplicates[8] = duplicates[7];
    let duplicate_refs: Vec<_> = duplicates.iter().map(|key| key.as_slice()).collect();
    assert!(decode_pool_initialization(&stabble::PROGRAM_ID, &data, &duplicate_refs).is_none());
}
