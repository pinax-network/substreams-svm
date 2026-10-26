use dex_pools::{
    decode_pool_initialization, project,
    solana::{base58, pb::sf::solana::r#type::v1 as pb},
    EvidencePosition, Protocol,
};
use solana_program::pubkey::Pubkey;
use substreams_solana_idls::{
    byreal::clmm as byreal,
    darklake, moonshot, pancakeswap, phoenix, plasma,
    raydium::launchpad,
    serum,
    spl::{token, token_2022},
};

type Key = [u8; 32];
struct Fixture {
    name: &'static str,
    program: Key,
    data: Vec<u8>,
    accounts: Vec<Key>,
    protocol: Protocol,
    address: Key,
    mints: [Key; 2],
    vaults: [Option<Key>; 2],
    programs: [Option<Key>; 2],
    decimals: [Option<u8>; 2],
    native: [bool; 2],
}

fn key(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}
fn public(key: Key) -> Pubkey {
    Pubkey::new_from_array(key)
}
fn accounts(value: &impl borsh::BorshSerialize) -> Vec<Key> {
    let bytes = borsh::to_vec(value).unwrap();
    assert_eq!(bytes.len() % 32, 0);
    bytes.chunks_exact(32).map(|bytes| bytes.try_into().unwrap()).collect()
}
fn data(tag: &[u8], value: &impl borsh::BorshSerialize) -> Vec<u8> {
    let mut result = tag.to_vec();
    result.extend(borsh::to_vec(value).unwrap());
    result
}

fn byreal_fixture() -> Fixture {
    Fixture {
        name: "byreal_create_pool",
        program: byreal::PROGRAM_ID,
        protocol: Protocol::Byreal,
        accounts: accounts(&byreal::accounts::CreatePoolAccounts {
            pool_creator: key(1),
            pool_manager: key(2),
            admin_group: key(3),
            amm_config: key(4),
            pool_state: key(5),
            offchain_reward_config: key(6),
            token_mint_0: key(7),
            token_mint_1: key(8),
            token_vault_0: key(9),
            token_vault_1: key(10),
            observation_state: key(11),
            tick_array_bitmap: key(12),
            token_program_0: public(token::PROGRAM_ID),
            token_program_1: public(token_2022::PROGRAM_ID),
            system_program: Pubkey::default(),
            rent: key(16),
        }),
        data: data(
            &byreal::instructions::CREATE_POOL,
            &byreal::instructions::CreatePoolInstruction {
                sqrt_price_x64: 1 << 64,
                open_time: 123,
            },
        ),
        address: [5; 32],
        mints: [[7; 32], [8; 32]],
        vaults: [Some([9; 32]), Some([10; 32])],
        programs: [Some(token::PROGRAM_ID), Some(token_2022::PROGRAM_ID)],
        decimals: [None, None],
        native: [false, false],
    }
}
fn pancake_fixture() -> Fixture {
    Fixture {
        name: "pancake_create_pool",
        program: pancakeswap::PROGRAM_ID,
        protocol: Protocol::Pancakeswap,
        accounts: accounts(&pancakeswap::accounts::CreatePoolAccounts {
            pool_creator: key(20),
            amm_config: key(21),
            pool_state: key(22),
            token_mint_0: key(23),
            token_mint_1: key(24),
            token_vault_0: key(25),
            token_vault_1: key(26),
            observation_state: key(27),
            tick_array_bitmap: key(28),
            token_program_0: public(token::PROGRAM_ID),
            token_program_1: public(token_2022::PROGRAM_ID),
            system_program: Pubkey::default(),
            rent: key(32),
        }),
        data: data(
            &pancakeswap::instructions::CREATE_POOL,
            &pancakeswap::instructions::CreatePoolInstruction {
                sqrt_price_x64: 1 << 64,
                open_time: 123,
            },
        ),
        address: [22; 32],
        mints: [[23; 32], [24; 32]],
        vaults: [Some([25; 32]), Some([26; 32])],
        programs: [Some(token::PROGRAM_ID), Some(token_2022::PROGRAM_ID)],
        decimals: [None, None],
        native: [false, false],
    }
}

fn launchpad_fixture(variant: u8) -> Fixture {
    use launchpad::instructions::*;
    let mint = MintParams {
        decimals: 0,
        name: "creation fixture".into(),
        symbol: "POOL".into(),
        uri: "https://example.invalid/pool".into(),
    };
    let curve = CurveParams::Constant {
        data: launchpad::anchor_cpi_event::ConstantCurve {
            supply: 1_000,
            total_base_sell: 800,
            total_quote_fund_raising: 200,
            migrate_type: 0,
        },
    };
    let vesting = VestingParams {
        total_locked_amount: 10,
        cliff_period: 20,
        unlock_period: 30,
    };
    let base_program = if variant == 2 { token_2022::PROGRAM_ID } else { token::PROGRAM_ID };
    let pool = 45 + variant;
    let (name, accounts, data) = match variant {
        0 => (
            "launchpad_initialize",
            accounts(&launchpad::accounts::InitializeAccounts {
                payer: key(40),
                creator: key(41),
                global_config: key(42),
                platform_config: key(43),
                authority: key(44),
                pool_state: key(pool),
                base_mint: key(48),
                quote_mint: key(49),
                base_vault: key(50),
                quote_vault: key(51),
                metadata_account: key(52),
                base_token_program: public(base_program),
                quote_token_program: public(token::PROGRAM_ID),
                metadata_program: key(55),
                system_program: Pubkey::default(),
                rent_program: key(57),
                event_authority: key(58),
                program: public(launchpad::PROGRAM_ID),
            }),
            data(
                &INITIALIZE,
                &InitializeInstruction {
                    base_mint_param: mint,
                    curve_param: curve,
                    vesting_param: vesting,
                },
            ),
        ),
        1 => (
            "launchpad_initialize_v2",
            accounts(&launchpad::accounts::InitializeV2Accounts {
                payer: key(40),
                creator: key(41),
                global_config: key(42),
                platform_config: key(43),
                authority: key(44),
                pool_state: key(pool),
                base_mint: key(48),
                quote_mint: key(49),
                base_vault: key(50),
                quote_vault: key(51),
                metadata_account: key(52),
                base_token_program: public(base_program),
                quote_token_program: public(token::PROGRAM_ID),
                metadata_program: key(55),
                system_program: Pubkey::default(),
                rent_program: key(57),
                event_authority: key(58),
                program: public(launchpad::PROGRAM_ID),
            }),
            data(
                &INITIALIZE_V2,
                &InitializeV2Instruction {
                    base_mint_param: mint,
                    curve_param: curve,
                    vesting_param: vesting,
                    amm_fee_on: AmmCreatorFeeOn::QuoteToken,
                },
            ),
        ),
        2 => (
            "launchpad_initialize_token2022",
            accounts(&launchpad::accounts::InitializeWithToken2022Accounts {
                payer: key(40),
                creator: key(41),
                global_config: key(42),
                platform_config: key(43),
                authority: key(44),
                pool_state: key(pool),
                base_mint: key(48),
                quote_mint: key(49),
                base_vault: key(50),
                quote_vault: key(51),
                base_token_program: public(base_program),
                quote_token_program: public(token::PROGRAM_ID),
                system_program: Pubkey::default(),
                event_authority: key(58),
                program: public(launchpad::PROGRAM_ID),
            }),
            data(
                &INITIALIZE_WITH_TOKEN_2022,
                &InitializeWithToken2022Instruction {
                    base_mint_param: mint,
                    curve_param: curve,
                    vesting_param: vesting,
                    amm_fee_on: AmmCreatorFeeOn::BothToken,
                    transfer_fee_extension_param: Some(TransferFeeExtensionParams {
                        transfer_fee_basis_points: 25,
                        maximum_fee: 100,
                    }),
                },
            ),
        ),
        _ => unreachable!(),
    };
    Fixture {
        name,
        program: launchpad::PROGRAM_ID,
        protocol: Protocol::RaydiumLaunchpad,
        accounts,
        data,
        address: [pool; 32],
        mints: [[48; 32], [49; 32]],
        vaults: [Some([50; 32]), Some([51; 32])],
        programs: [Some(base_program), Some(token::PROGRAM_ID)],
        decimals: [Some(0), None],
        native: [false, false],
    }
}

fn phoenix_fixture() -> Fixture {
    Fixture {
        name: "phoenix_initialize_market",
        program: phoenix::PROGRAM_ID,
        protocol: Protocol::Phoenix,
        accounts: accounts(&phoenix::accounts::InitializeMarketAccounts {
            phoenix_program: public(phoenix::PROGRAM_ID),
            log_authority: key(60),
            market: key(61),
            market_creator: key(62),
            base_mint: key(63),
            quote_mint: key(64),
            base_vault: key(65),
            quote_vault: key(66),
            system_program: Pubkey::default(),
            token_program: public(token::PROGRAM_ID),
        }),
        data: data(
            &[phoenix::instructions::INITIALIZE_MARKET],
            &phoenix::instructions::InitializeMarketInstruction {
                market_size_params: phoenix::instructions::MarketSizeParams {
                    bids_size: 512,
                    asks_size: 512,
                    num_seats: 128,
                },
                num_quote_lots_per_quote_unit: 100,
                tick_size_in_quote_lots_per_base_unit: 1,
                num_base_lots_per_base_unit: 100,
                taker_fee_bps: 10,
                fee_collector: key(69),
                raw_base_units_per_base_unit: Some(10),
            },
        ),
        address: [61; 32],
        mints: [[63; 32], [64; 32]],
        vaults: [Some([65; 32]), Some([66; 32])],
        programs: [Some(token::PROGRAM_ID); 2],
        decimals: [None, None],
        native: [false, false],
    }
}
fn moonshot_fixture(currency: u8) -> Fixture {
    Fixture {
        name: "moonshot_token_mint",
        program: moonshot::PROGRAM_ID,
        protocol: Protocol::Moonshot,
        accounts: accounts(&moonshot::accounts::TokenMintAccounts {
            sender: key(70),
            backend_authority: key(71),
            curve_account: key(72),
            mint: key(73),
            mint_metadata: key(74),
            curve_token_account: key(75),
            config_account: key(76),
            token_program: public(token::PROGRAM_ID),
            associated_token_program: key(78),
            mpl_token_metadata: key(79),
            system_program: Pubkey::default(),
        }),
        data: data(
            &moonshot::instructions::TOKEN_MINT,
            &moonshot::instructions::TokenMintInstruction {
                name: "Moon fixture".into(),
                symbol: "MOON".into(),
                uri: "https://example.invalid/moon".into(),
                decimals: 6,
                collateral_currency: currency,
                amount: 1_000,
                curve_type: 0,
            },
        ),
        address: [72; 32],
        mints: [
            [73; 32],
            base58::decode("So11111111111111111111111111111111111111112").unwrap().try_into().unwrap(),
        ],
        vaults: [Some([75; 32]), None],
        programs: [Some(token::PROGRAM_ID), None],
        decimals: [Some(6), Some(9)],
        native: [false, true],
    }
}
fn darklake_fixture() -> Fixture {
    Fixture {
        name: "darklake_initialize_pool",
        program: darklake::PROGRAM_ID,
        protocol: Protocol::Darklake,
        accounts: accounts(&darklake::accounts::InitializePoolAccounts {
            user: key(81),
            pool: key(82),
            authority: key(83),
            amm_config: key(84),
            token_mint_x: key(85),
            token_mint_y: key(86),
            token_mint_wsol: key(87),
            token_mint_lp: key(88),
            metadata_account: key(89),
            metadata_account_x: key(90),
            metadata_account_y: key(91),
            user_token_account_x: key(92),
            user_token_account_y: key(93),
            user_token_account_lp: key(94),
            pool_token_reserve_x: key(95),
            pool_token_reserve_y: key(96),
            pool_wsol_reserve: key(97),
            create_pool_fee_vault: key(98),
            mpl_program: key(99),
            system_program: Pubkey::default(),
            rent: key(101),
            associated_token_program: key(102),
            token_mint_x_program: public(token::PROGRAM_ID),
            token_mint_y_program: public(token_2022::PROGRAM_ID),
            token_program: public(token::PROGRAM_ID),
        }),
        data: data(
            &darklake::instructions::INITIALIZE_POOL,
            &darklake::instructions::InitializePoolInstruction {
                amount_x: 100,
                amount_y: 200,
                label: Some([3; 21]),
            },
        ),
        address: [82; 32],
        mints: [[85; 32], [86; 32]],
        vaults: [Some([95; 32]), Some([96; 32])],
        programs: [Some(token::PROGRAM_ID), Some(token_2022::PROGRAM_ID)],
        decimals: [None, None],
        native: [false, false],
    }
}
fn plasma_fixture() -> Fixture {
    Fixture {
        name: "plasma_initialize_pool",
        program: plasma::PROGRAM_ID,
        protocol: Protocol::Plasma,
        accounts: accounts(&plasma::accounts::InitializePoolAccounts {
            plasma_program: public(plasma::PROGRAM_ID),
            log_authority: key(100),
            pool: key(110),
            pool_creator: key(111),
            base_mint: key(112),
            quote_mint: key(113),
            base_vault: key(114),
            quote_vault: key(115),
            system_program: Pubkey::default(),
            token_program: public(token::PROGRAM_ID),
        }),
        data: data(
            &[plasma::instructions::INITIALIZE_POOL],
            &plasma::instructions::InitializePoolInstruction {
                lp_fee_in_bps: 25,
                protocol_lp_fee_allocation_in_pct: 10,
                fee_recipients_params: [
                    plasma::instructions::ProtocolFeeRecipientParams {
                        recipient: [116; 32],
                        shares: 50,
                    },
                    plasma::instructions::ProtocolFeeRecipientParams {
                        recipient: [117; 32],
                        shares: 30,
                    },
                    plasma::instructions::ProtocolFeeRecipientParams {
                        recipient: [118; 32],
                        shares: 20,
                    },
                ],
                num_slots_to_vest_lp_shares: Some(100),
            },
        ),
        address: [110; 32],
        mints: [[112; 32], [113; 32]],
        vaults: [Some([114; 32]), Some([115; 32])],
        programs: [Some(token::PROGRAM_ID); 2],
        decimals: [None, None],
        native: [false, false],
    }
}
fn serum_fixture() -> Fixture {
    let mut keys = accounts(&serum::accounts::InitializeMarketAccounts {
        market_to_initialize: key(120),
        request_queue: key(121),
        event_queue: key(122),
        bids: key(123),
        asks: key(124),
        spl_token_account_coin: key(125),
        spl_token_account_price: key(126),
        coin_currency_mint: key(127),
        price_currency_mint: key(128),
    });
    keys.push([129; 32]); // Required rent account, after the shared helper's nine identity accounts.
    Fixture {
        name: "serum_initialize_market",
        program: serum::PROGRAM_ID,
        protocol: Protocol::Serum,
        accounts: keys,
        data: data(
            &[0, 0, 0, 0, 0],
            &serum::instructions::InitializeMarketInstruction {
                coin_lot_size: 100,
                pc_lot_size: 10,
                fee_rate_bps: 25,
                vault_signer_nonce: 1,
                pc_dust_threshold: 100,
            },
        ),
        address: [120; 32],
        mints: [[127; 32], [128; 32]],
        vaults: [Some([125; 32]), Some([126; 32])],
        programs: [None, None],
        decimals: [None, None],
        native: [false, false],
    }
}
fn fixtures() -> Vec<Fixture> {
    vec![
        byreal_fixture(),
        pancake_fixture(),
        launchpad_fixture(0),
        launchpad_fixture(1),
        launchpad_fixture(2),
        phoenix_fixture(),
        moonshot_fixture(0),
        darklake_fixture(),
        plasma_fixture(),
        serum_fixture(),
    ]
}
fn candidate(fixture: &Fixture) -> Option<dex_pools::Candidate> {
    decode_pool_initialization(
        &fixture.program,
        &fixture.data,
        &fixture.accounts.iter().map(|key| key.as_slice()).collect::<Vec<_>>(),
    )
}

fn block(fixtures: &[Fixture]) -> pb::Block {
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
        logs.extend([format!("Program {program} invoke [1]"), format!("Program {program} success")]);
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
                ..Default::default()
            }
            .into(),
        }],
        ..Default::default()
    }
}

#[test]
fn typed_initializers_preserve_all_ten_variant_identities_and_token_order() {
    for fixture in fixtures() {
        let value = candidate(&fixture).unwrap_or_else(|| panic!("{} must decode", fixture.name));
        assert_eq!(value.protocol, fixture.protocol, "{}", fixture.name);
        assert_eq!(value.program_id, fixture.program, "{}", fixture.name);
        assert_eq!(value.address, fixture.address, "{}", fixture.name);
        assert_eq!(value.tokens.len(), 2, "{}", fixture.name);
        for (i, token) in value.tokens.iter().enumerate() {
            assert_eq!(token.mint, fixture.mints[i], "{} token{i}", fixture.name);
            assert_eq!(token.vault, fixture.vaults[i], "{} token{i}", fixture.name);
            assert_eq!(token.token_program, fixture.programs[i], "{} token{i}", fixture.name);
            assert_eq!(token.decimals, fixture.decimals[i], "{} token{i}", fixture.name);
            assert_eq!(token.is_native, fixture.native[i], "{} token{i}", fixture.name);
        }
    }
}

#[test]
fn whole_block_projection_retains_every_variant_and_requires_committed_trace() {
    let fixtures = fixtures();
    let input = block(&fixtures);
    let projection = project(&input).unwrap();
    assert_eq!(projection.pools.len(), 10);
    for (i, fixture) in fixtures.iter().enumerate() {
        let pool = projection.pools.iter().find(|pool| pool.address == fixture.address).unwrap();
        assert_eq!(pool.tokens, candidate(fixture).unwrap().tokens);
        assert_eq!(pool.creation.instruction.instruction_index, i as u32);
        assert_eq!(pool.creation.evidence, EvidencePosition::Initialization);
    }
    let mut invalid = input.clone();
    invalid.transactions[0].meta.as_option_mut().unwrap().log_messages.pop();
    assert!(project(&invalid).unwrap().pools.is_empty());
    invalid = input;
    invalid.transactions[0].meta.as_option_mut().unwrap().err = pb::TransactionError::default().into();
    assert!(project(&invalid).unwrap().pools.is_empty());
}

#[test]
fn every_variant_rejects_incomplete_extra_unknown_and_wrong_program_inputs() {
    for fixture in fixtures() {
        let refs = fixture.accounts.iter().map(|key| key.as_slice()).collect::<Vec<_>>();
        assert!(
            decode_pool_initialization(&fixture.program, &fixture.data, &refs[..refs.len() - 1]).is_none(),
            "{} missing accounts",
            fixture.name
        );
        assert!(
            decode_pool_initialization(&fixture.program, &fixture.data[..fixture.data.len() - 1], &refs).is_none(),
            "{} truncated",
            fixture.name
        );
        let mut extra = fixture.data.clone();
        extra.push(0);
        assert!(
            decode_pool_initialization(&fixture.program, &extra, &refs).is_none(),
            "{} trailing payload",
            fixture.name
        );
        assert!(
            decode_pool_initialization(&fixture.program, &[255; 8], &refs).is_none(),
            "{} unknown discriminator",
            fixture.name
        );
        assert!(
            decode_pool_initialization(&[200; 32], &fixture.data, &refs).is_none(),
            "{} wrong program",
            fixture.name
        );
    }
}

#[test]
fn versioned_serum_and_moonshot_currency_are_not_guessed() {
    let mut serum = serum_fixture();
    serum.data[0] = 1;
    assert!(candidate(&serum).is_none(), "only canonical Serum version0 is supported");
    assert!(candidate(&moonshot_fixture(1)).is_none(), "future collateral must not be labeled SOL");
    for mut fixture in [phoenix_fixture(), plasma_fixture()] {
        fixture.accounts[0] = [200; 32];
        assert!(candidate(&fixture).is_none(), "self-program account must match the invoked program");
    }
    for i in 0..3 {
        let mut fixture = launchpad_fixture(i);
        *fixture.accounts.last_mut().unwrap() = [200; 32];
        assert!(candidate(&fixture).is_none());
    }
}
