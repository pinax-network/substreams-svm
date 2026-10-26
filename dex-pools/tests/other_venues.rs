use dex_pools::{
    decode_pool_initialization, project,
    solana::{base58, pb::sf::solana::r#type::v1 as pb},
    Candidate, EvidencePosition, Protocol,
};
use sha2::{Digest, Sha256};
use solana_program::pubkey::Pubkey;
use substreams_solana_idls::{
    bonkswap, boop, dumpfun, openbook,
    pumpfun::bonding_curve as pump,
    spl::{token, token_2022},
};

struct Fixture {
    program: [u8; 32],
    accounts: Vec<[u8; 32]>,
    data: Vec<u8>,
}

fn key(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}

fn accounts(value: &impl borsh::BorshSerialize) -> Vec<[u8; 32]> {
    let data = borsh::to_vec(value).unwrap();
    assert_eq!(data.len() % 32, 0);
    data.chunks_exact(32).map(|key| key.try_into().unwrap()).collect()
}

fn tagged(name: &str, value: &impl borsh::BorshSerialize) -> Vec<u8> {
    let mut data = Sha256::digest(name.as_bytes())[..8].to_vec();
    data.extend(borsh::to_vec(value).unwrap());
    data
}

fn bonkswap_fixture() -> Fixture {
    use bonkswap::{accounts::CreatePoolAccounts, instructions::*};
    Fixture {
        program: bonkswap::PROGRAM_ID,
        accounts: accounts(&CreatePoolAccounts {
            state: key(1),
            pool: key(2),
            token_x: key(3),
            token_y: key(4),
            pool_x_account: key(5),
            pool_y_account: key(6),
            admin_x_account: key(7),
            admin_y_account: key(8),
            admin: key(9),
            project_owner: key(10),
            program_authority: key(11),
            system_program: Pubkey::default(),
            token_program: Pubkey::new_from_array(token::PROGRAM_ID),
            rent: key(14),
        }),
        data: {
            let mut data = CREATE_POOL.to_vec();
            data.extend(
                borsh::to_vec(&CreatePoolInstruction {
                    lp_fee: FixedPoint { v: 1 },
                    buyback_fee: FixedPoint { v: 2 },
                    project_fee: FixedPoint { v: 3 },
                    mercanti_fee: FixedPoint { v: 4 },
                    initial_token_x: Token { v: 100 },
                    initial_token_y: Token { v: 200 },
                    bump: 255,
                })
                .unwrap(),
            );
            data
        },
    }
}

fn pump_fixture() -> Fixture {
    Fixture {
        program: pump::PROGRAM_ID,
        accounts: accounts(&pump::accounts::CreateAccounts {
            mint: key(21),
            mint_authority: key(22),
            bonding_curve: key(23),
            curve: key(24),
            global_state: key(25),
            metadata_program: key(26),
            metadata: key(27),
            user: key(28),
            system_program: Pubkey::default(),
            token_program: Pubkey::new_from_array(token::PROGRAM_ID),
            associated_token_program: key(31),
            rent: key(32),
            event_authority: key(33),
            program: Pubkey::new_from_array(pump::PROGRAM_ID),
        }),
        data: tagged(
            "global:create",
            &pump::instructions::CreateInstruction {
                name: "Pool fixture".into(),
                symbol: "POOL".into(),
                uri: "https://example.invalid/pool.json".into(),
                creator: key(28),
            },
        ),
    }
}

fn pump_v2_fixture(quote: Option<[u8; 32]>) -> Fixture {
    let mut keys = accounts(&pump::accounts::CreateV2Accounts {
        mint: key(110),
        mint_authority: key(111),
        bonding_curve: key(112),
        associated_bonding_curve: key(113),
        global: key(114),
        user: key(115),
        system_program: Pubkey::default(),
        token_program: Pubkey::new_from_array(token_2022::PROGRAM_ID),
        associated_token_program: key(118),
        mayhem_program_id: key(119),
        global_params: key(120),
        sol_vault: key(121),
        mayhem_state: key(122),
        mayhem_token_vault: key(123),
        event_authority: key(124),
        program: Pubkey::new_from_array(pump::PROGRAM_ID),
    });
    if let Some(program) = quote {
        keys.extend(accounts(&pump::accounts::CreateV2QuoteAccounts {
            quote_mint: key(160),
            associated_quote_bonding_curve: key(161),
            quote_token_program: Pubkey::new_from_array(program),
        }));
    }
    Fixture {
        program: pump::PROGRAM_ID,
        accounts: keys,
        data: tagged(
            "global:create_v2",
            &pump::instructions::CreateV2Instruction {
                name: "Current pool".into(),
                symbol: "CUR".into(),
                uri: "https://example.invalid/current".into(),
                creator: key(115),
                is_mayhem_mode: false,
                is_cashback_enabled: pump::instructions::OptionBool(false),
                creator_fee_bps: pump::instructions::OptionU64(0),
                is_holder_reward: pump::instructions::OptionBool(false),
            },
        ),
    }
}

fn boop_fixture(fallback: bool) -> Fixture {
    let accounts = accounts(&boop::accounts::DeployBondingCurveAccounts {
        mint: key(90),
        vault_authority: key(91),
        bonding_curve: key(if fallback { 92 } else { 93 }),
        bonding_curve_sol_vault: key(94),
        bonding_curve_vault: key(95),
        config: key(96),
        payer: key(97),
        system_program: Pubkey::default(),
        token_program: Pubkey::new_from_array(token::PROGRAM_ID),
        associated_token_program: key(99),
    });
    let mut data = if fallback {
        boop::instructions::DEPLOY_BONDING_CURVE_FALLBACK
    } else {
        boop::instructions::DEPLOY_BONDING_CURVE
    }
    .to_vec();
    if fallback {
        data.extend(borsh::to_vec(&boop::instructions::DeployBondingCurveFallbackInstruction { creator: key(97), salt: 2 }).unwrap());
    } else {
        data.extend(borsh::to_vec(&boop::instructions::DeployBondingCurveInstruction { creator: key(97), salt: 1 }).unwrap());
    }
    Fixture {
        program: boop::PROGRAM_ID,
        accounts,
        data,
    }
}
fn dumpfun_fixture() -> Fixture {
    let accounts = accounts(&dumpfun::accounts::MintAccounts {
        payer: key(170),
        metadata_account: key(171),
        mint_account: key(172),
        liquidity_pool: key(173),
        liquidity_pool_authority: key(174),
        liquidity_pool_token_account: key(175),
        token_metadata_program: key(176),
        token_program: Pubkey::new_from_array(token::PROGRAM_ID),
        associated_token_program: key(178),
        system_program: Pubkey::default(),
        rent: key(180),
    });
    let mut data = dumpfun::instructions::MINT.to_vec();
    data.extend(
        borsh::to_vec(&dumpfun::instructions::MintInstruction {
            token_name: "Dump fixture".into(),
            token_symbol: "DUMP".into(),
            token_uri: "https://example.invalid/dump".into(),
            sell_lock_period: 10,
            virtual_sol_reserve: 100,
            ramping_limits: vec![dumpfun::instructions::RampingLimit {
                start_sec: 0,
                end_sec: 10,
                limit_bps: 100,
            }],
        })
        .unwrap(),
    );
    Fixture {
        program: dumpfun::PROGRAM_ID,
        accounts,
        data,
    }
}

fn openbook_fixture() -> Fixture {
    Fixture {
        program: openbook::PROGRAM_ID,
        accounts: accounts(&openbook::accounts::CreateMarketAccounts {
            market: key(41),
            market_authority: key(42),
            bids: key(43),
            asks: key(44),
            event_heap: key(45),
            payer: key(46),
            market_base_vault: key(47),
            market_quote_vault: key(48),
            base_mint: key(49),
            quote_mint: key(50),
            system_program: Pubkey::default(),
            token_program: Pubkey::new_from_array(token::PROGRAM_ID),
            associated_token_program: key(53),
            // Anchor optional accounts use the program as the absent sentinel.
            oracle_a: Pubkey::new_from_array(openbook::PROGRAM_ID),
            oracle_b: Pubkey::new_from_array(openbook::PROGRAM_ID),
            collect_fee_admin: key(56),
            open_orders_admin: Pubkey::new_from_array(openbook::PROGRAM_ID),
            consume_events_admin: Pubkey::new_from_array(openbook::PROGRAM_ID),
            close_market_admin: Pubkey::new_from_array(openbook::PROGRAM_ID),
            event_authority: key(60),
            program: Pubkey::new_from_array(openbook::PROGRAM_ID),
        }),
        // The existing shared IDL has no CreateMarket argument struct. This
        // intentionally exercises its discriminator-only Candidate boundary;
        // it is not a serialized on-chain-valid CreateMarket instruction.
        data: openbook::instructions::CREATE_MARKET.to_vec(),
    }
}

fn decode(fixture: &Fixture) -> Option<Candidate> {
    let account_refs: Vec<_> = fixture.accounts.iter().map(|key| key.as_slice()).collect();
    decode_pool_initialization(&fixture.program, &fixture.data, &account_refs)
}

fn block(fixtures: &[Fixture], succeeded: bool) -> pb::Block {
    let mut keys = vec![vec![254; 32]];
    let mut index = |key: &[u8]| {
        if let Some(index) = keys.iter().position(|value| value == key) {
            index as u32
        } else {
            keys.push(key.to_vec());
            (keys.len() - 1) as u32
        }
    };
    let mut instructions = vec![];
    let mut logs = vec![];
    for fixture in fixtures {
        instructions.push(pb::CompiledInstruction {
            program_id_index: index(&fixture.program),
            accounts: fixture.accounts.iter().map(|key| index(key) as u8).collect(),
            data: fixture.data.clone(),
        });
        let program = base58::encode(fixture.program);
        logs.push(format!("Program {program} invoke [1]"));
        logs.push(if succeeded {
            format!("Program {program} success")
        } else {
            format!("Program {program} failed: custom program error: 0x1")
        });
    }
    pb::Block {
        slot: 123,
        parent_slot: 120,
        blockhash: base58::encode([55; 32]),
        previous_blockhash: base58::encode([54; 32]),
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
                    instructions,
                    ..Default::default()
                }
                .into(),
            }
            .into(),
            meta: pb::TransactionStatusMeta {
                log_messages: logs,
                err: (!succeeded).then(pb::TransactionError::default).into(),
                ..Default::default()
            }
            .into(),
        }],
        ..Default::default()
    }
}

#[test]
fn bonkswap_preserves_x_y_identity_and_unknown_decimals() {
    let fixture = bonkswap_fixture();
    let candidate = decode(&fixture).unwrap();
    assert_eq!(candidate.protocol, Protocol::Bonkswap);
    assert_eq!(candidate.address, [2; 32]);
    assert_eq!(candidate.program_id, bonkswap::PROGRAM_ID);
    assert_eq!(candidate.tokens.len(), 2);
    for (token, mint, vault) in [(&candidate.tokens[0], 3, 5), (&candidate.tokens[1], 4, 6)] {
        assert_eq!(token.mint, [mint; 32]);
        assert_eq!(token.vault, Some([vault; 32]));
        assert_eq!(token.decimals, None);
        assert_eq!(token.token_program, Some(token::PROGRAM_ID));
        assert!(!token.is_native);
    }
}

#[test]
fn pump_historical_create_exposes_native_sol_without_fake_token_vault() {
    let candidate = decode(&pump_fixture()).unwrap();
    assert_eq!(candidate.protocol, Protocol::PumpfunBondingCurve);
    assert_eq!(candidate.address, [23; 32]);
    assert_eq!(candidate.tokens[0].mint, [21; 32]);
    assert_eq!(candidate.tokens[0].vault, Some([24; 32]));
    assert_eq!(candidate.tokens[0].decimals, None);
    assert_eq!(candidate.tokens[0].token_program, Some(token::PROGRAM_ID));
    assert!(!candidate.tokens[0].is_native);
    assert_eq!(base58::encode(candidate.tokens[1].mint), "So11111111111111111111111111111111111111112");
    assert!(candidate.tokens[1].is_native);
    assert_eq!(candidate.tokens[1].decimals, Some(9));
    assert_eq!(candidate.tokens[1].vault, None);
    assert_eq!(candidate.tokens[1].token_program, None);
}

#[test]
fn current_pump_creation_distinguishes_native_and_explicit_token_quotes() {
    for quote in [None, Some(token::PROGRAM_ID), Some(token_2022::PROGRAM_ID)] {
        let fixture = pump_v2_fixture(quote);
        let candidate = decode(&fixture).unwrap();
        assert_eq!(candidate.address, [112; 32]);
        assert_eq!(candidate.tokens[0].mint, [110; 32]);
        assert_eq!(candidate.tokens[0].vault, Some([113; 32]));
        assert_eq!(candidate.tokens[0].token_program, Some(token_2022::PROGRAM_ID));
        if let Some(program) = quote {
            assert_eq!(candidate.tokens[1].mint, [160; 32]);
            assert_eq!(candidate.tokens[1].vault, Some([161; 32]));
            assert_eq!(candidate.tokens[1].token_program, Some(program));
            assert_eq!(candidate.tokens[1].decimals, None);
            assert!(!candidate.tokens[1].is_native);
        } else {
            assert!(candidate.tokens[1].is_native);
            assert_eq!(candidate.tokens[1].token_program, None);
            assert_eq!(candidate.tokens[1].vault, None);
        }
        assert_eq!(project(&block(&[fixture], true)).unwrap().pools[0].tokens, candidate.tokens);
    }
    for extra in 1..=2 {
        let mut incomplete = pump_v2_fixture(None);
        incomplete.accounts.extend(vec![[160; 32]; extra]);
        assert!(decode(&incomplete).is_none(), "partial token-quote accounts must not fall back to SOL");
    }
    let mut chain = pump_v2_fixture(Some(token::PROGRAM_ID));
    chain.accounts.extend((181..201).map(|n| [n; 32]));
    assert!(chain.accounts.len() > 32);
    assert_eq!(
        decode(&chain).unwrap().tokens[1].mint,
        [160; 32],
        "quote chain accounts must not change quote identity"
    );
}

#[test]
fn boop_both_deploy_variants_and_dumpfun_mint_keep_native_quote_semantics() {
    for (fixture, protocol, pool, mint, vault) in [
        (boop_fixture(false), Protocol::Boop, 93, 90, 95),
        (boop_fixture(true), Protocol::Boop, 92, 90, 95),
        (dumpfun_fixture(), Protocol::Dumpfun, 173, 172, 175),
    ] {
        let candidate = decode(&fixture).unwrap();
        assert_eq!(candidate.protocol, protocol);
        assert_eq!(candidate.address, [pool; 32]);
        assert_eq!(candidate.tokens[0].mint, [mint; 32]);
        assert_eq!(candidate.tokens[0].vault, Some([vault; 32]));
        assert_eq!(candidate.tokens[0].decimals, None);
        assert_eq!(candidate.tokens[0].token_program, Some(token::PROGRAM_ID));
        assert!(!candidate.tokens[0].is_native);
        assert!(candidate.tokens[1].is_native);
        assert_eq!(candidate.tokens[1].vault, None);
        assert_eq!(candidate.tokens[1].token_program, None);
        assert_eq!(project(&block(&[fixture], true)).unwrap().pools[0].tokens, candidate.tokens);
    }
}

#[test]
fn openbook_candidate_is_market_base_quote_without_argument_validation() {
    let candidate = decode(&openbook_fixture()).unwrap();
    assert_eq!(candidate.protocol, Protocol::Openbook);
    assert_eq!(candidate.address, [41; 32]);
    assert_eq!(candidate.tokens[0].mint, [49; 32]);
    assert_eq!(candidate.tokens[0].vault, Some([47; 32]));
    assert_eq!(candidate.tokens[1].mint, [50; 32]);
    assert_eq!(candidate.tokens[1].vault, Some([48; 32]));
    assert!(candidate.tokens.iter().all(|value| value.decimals.is_none() && !value.is_native));
    // A discriminator-only raw candidate is deliberately not evidence that a
    // program accepted the payload: a failed execution emits no committed Pool.
    assert!(project(&block(&[openbook_fixture()], false)).unwrap().pools.is_empty());
}

#[test]
fn full_projection_retains_both_creations_and_requires_execution_evidence() {
    let input = block(&[bonkswap_fixture(), pump_fixture()], true);
    let projection = project(&input).unwrap();
    assert_eq!(projection.pools.len(), 2);
    assert_eq!(projection.pools[0].address, [2; 32]);
    assert_eq!(projection.pools[1].address, [23; 32]);
    assert_eq!(projection.pools[0].creation.instruction.instruction_index, 0);
    assert_eq!(projection.pools[1].creation.instruction.instruction_index, 1);
    assert!(projection.pools.iter().all(|pool| pool.creation.evidence == EvidencePosition::Initialization));
    let mut truncated = input;
    truncated.transactions[0].meta.as_option_mut().unwrap().log_messages.pop();
    assert!(project(&truncated).unwrap().pools.is_empty());
    assert!(project(&block(&[bonkswap_fixture(), pump_fixture()], false)).unwrap().pools.is_empty());
}

#[test]
fn truncated_typed_payloads_missing_accounts_wrong_programs_and_unknown_variants_are_rejected() {
    for mut fixture in [
        bonkswap_fixture(),
        pump_fixture(),
        openbook_fixture(),
        boop_fixture(false),
        boop_fixture(true),
        dumpfun_fixture(),
    ] {
        let mut missing = Fixture {
            program: fixture.program,
            accounts: fixture.accounts[..fixture.accounts.len() - 1].to_vec(),
            data: fixture.data.clone(),
        };
        assert!(decode(&missing).is_none());
        missing.accounts = fixture.accounts.clone();
        missing.program = [99; 32];
        assert!(decode(&missing).is_none());
        fixture.data.pop();
        assert!(decode(&fixture).is_none());
        fixture.data = vec![255; 8];
        assert!(decode(&fixture).is_none());
    }
    let mut pump = pump_fixture();
    pump.accounts[13] = [99; 32];
    assert!(decode(&pump).is_none());
    let mut pump = pump_fixture();
    pump.data[..8].copy_from_slice(&Sha256::digest(b"global:create_v2")[..8]);
    assert!(decode(&pump).is_none(), "create_v2 must not use historical payload or account layout");
}
