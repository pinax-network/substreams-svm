use base64::{engine::general_purpose::STANDARD, Engine};
use dex_pools::{
    project, project_with_limits,
    solana::{base58, pb::sf::solana::r#type::v1 as pb},
    EvidencePosition, Limits, ProjectError, Protocol,
};
use sha2::{Digest, Sha256};
use solana_program::pubkey::Pubkey;
use substreams_solana_idls::{orca::whirlpool as orca, pumpfun::amm as pump, spl::token};

struct Call {
    program: [u8; 32],
    accounts: Vec<[u8; 32]>,
    data: Vec<u8>,
    logs: Vec<String>,
    children: Vec<Call>,
    failed: bool,
}
impl Call {
    fn new(program: [u8; 32], accounts: Vec<[u8; 32]>, data: Vec<u8>) -> Self {
        Self {
            program,
            accounts,
            data,
            logs: vec![],
            children: vec![],
            failed: false,
        }
    }
}
fn tagged(name: &str, value: &impl borsh::BorshSerialize) -> Vec<u8> {
    let mut data = Sha256::digest(name.as_bytes())[..8].to_vec();
    data.extend(borsh::to_vec(value).unwrap());
    data
}
fn key(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}
fn orca_call(pool: u8) -> Call {
    let mut accounts: Vec<_> = (1..=14).map(|n| [n; 32]).collect();
    accounts[6] = [pool; 32];
    accounts[10] = token::PROGRAM_ID;
    accounts[11] = token::PROGRAM_ID;
    let data = tagged(
        "global:initialize_pool_v2",
        &orca::instructions::InitializePoolV2Instruction {
            tick_spacing: 64,
            initial_sqrt_price: 1u128 << 64,
        },
    );
    let event = orca::events::PoolInitialized {
        whirlpool: key(pool),
        whirlpools_config: key(1),
        token_mint_a: key(2),
        token_mint_b: key(3),
        tick_spacing: 64,
        token_program_a: Pubkey::new_from_array(token::PROGRAM_ID),
        token_program_b: Pubkey::new_from_array(token::PROGRAM_ID),
        decimals_a: 6,
        decimals_b: 9,
        initial_sqrt_price: 1u128 << 64,
    };
    let mut call = Call::new(orca::PROGRAM_ID, accounts, data);
    call.logs
        .push(format!("Program data: {}", STANDARD.encode(tagged("event:PoolInitialized", &event))));
    call
}
fn pump_call(pool: u8) -> Call {
    let mut accounts: Vec<_> = (1..=18).map(|n| [n; 32]).collect();
    accounts[0] = [pool; 32];
    accounts[13] = token::PROGRAM_ID;
    accounts[14] = token::PROGRAM_ID;
    accounts[17] = pump::PROGRAM_ID;
    let data = tagged(
        "global:create_pool",
        &pump::instructions::CreatePoolInstructionV2 {
            index: 1,
            base_amount_in: 10,
            quote_amount_in: 20,
            coin_creator: key(44),
        },
    );
    let event = pump::events::CreatePoolEventV2 {
        timestamp: 1,
        index: 1,
        creator: key(3),
        base_mint: key(4),
        quote_mint: key(5),
        base_mint_decimals: 6,
        quote_mint_decimals: 9,
        base_amount_in: 10,
        quote_amount_in: 20,
        pool_base_amount: 10,
        pool_quote_amount: 20,
        minimum_liquidity: 1,
        initial_liquidity: 2,
        lp_token_amount_out: 1,
        pool_bump: 1,
        pool: key(pool),
        lp_mint: key(6),
        user_base_token_account: key(7),
        user_quote_token_account: key(8),
        coin_creator: key(44),
    };
    let mut event_data = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
    event_data.extend(tagged("event:CreatePoolEvent", &event));
    let mut call = Call::new(pump::PROGRAM_ID, accounts, data);
    call.children.push(Call::new(pump::PROGRAM_ID, vec![[17; 32]], event_data));
    call
}
fn block(calls: Vec<Call>) -> pb::Block {
    fn idx(keys: &mut Vec<Vec<u8>>, key: [u8; 32]) -> u32 {
        if let Some(i) = keys.iter().position(|k| k.as_slice() == key) {
            return i as u32;
        }
        keys.push(key.to_vec());
        (keys.len() - 1) as u32
    }
    fn walk(call: &Call, depth: u32, keys: &mut Vec<Vec<u8>>, inner: &mut Vec<pb::InnerInstruction>, logs: &mut Vec<String>) {
        let program = base58::encode(call.program);
        logs.push(format!("Program {program} invoke [{depth}]"));
        for child in &call.children {
            inner.push(pb::InnerInstruction {
                program_id_index: idx(keys, child.program),
                accounts: child.accounts.iter().map(|a| idx(keys, *a) as u8).collect(),
                data: child.data.clone(),
                stack_height: Some(depth + 1),
            });
            walk(child, depth + 1, keys, inner, logs);
        }
        logs.extend(call.logs.iter().cloned());
        logs.push(if call.failed {
            format!("Program {program} failed: custom program error: 0x1")
        } else {
            format!("Program {program} success")
        });
    }
    let mut keys = vec![vec![254; 32]];
    let mut instructions = vec![];
    let mut groups = vec![];
    let mut logs = vec![];
    for (i, call) in calls.iter().enumerate() {
        instructions.push(pb::CompiledInstruction {
            program_id_index: idx(&mut keys, call.program),
            accounts: call.accounts.iter().map(|a| idx(&mut keys, *a) as u8).collect(),
            data: call.data.clone(),
        });
        let mut inner = vec![];
        walk(call, 1, &mut keys, &mut inner, &mut logs);
        if !inner.is_empty() {
            groups.push(pb::InnerInstructions {
                index: i as u32,
                instructions: inner,
            });
        }
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
                inner_instructions: groups,
                log_messages: logs,
                ..Default::default()
            }
            .into(),
        }],
        ..Default::default()
    }
}
#[test]
fn admits_orca_and_pump_with_exact_creation_provenance() {
    let result = project(&block(vec![orca_call(50), pump_call(51)])).unwrap();
    assert_eq!(result.pools.len(), 2);
    let orca = &result.pools[0];
    assert_eq!(orca.protocol, Protocol::OrcaWhirlpool);
    assert_eq!(orca.address, [50; 32]);
    assert_eq!(orca.tokens[0].vault, Some([8; 32]));
    assert_eq!(orca.tokens[0].decimals, Some(6));
    assert_eq!(orca.creation.instruction.instruction_index, 0);
    assert!(matches!(orca.creation.evidence, EvidencePosition::Log { log_index: 1 }));
    let pump = &result.pools[1];
    assert_eq!(pump.protocol, Protocol::PumpfunAmm);
    assert_eq!(pump.creation.instruction.instruction_index, 1);
    assert!(matches!(pump.creation.evidence,EvidencePosition::Instruction(p) if p.inner_instruction_index==Some(0)));
    assert_eq!(pump.creation.signature, [7; 64]);
    assert_eq!(pump.creation.slot, 123);
}
#[test]
fn rejected_transaction_and_caught_failed_ancestor_never_create_pools() {
    let mut failed = block(vec![orca_call(50)]);
    failed.transactions[0].meta.as_option_mut().unwrap().err = pb::TransactionError::default().into();
    assert!(project(&failed).unwrap().pools.is_empty());
    let mut ancestor = Call::new([88; 32], vec![], vec![]);
    ancestor.children.push(pump_call(51));
    ancestor.failed = true;
    let mut outer = Call::new([89; 32], vec![], vec![]);
    outer.children.push(ancestor);
    assert!(project(&block(vec![outer])).unwrap().pools.is_empty());
    let mut orca = orca_call(50);
    orca.failed = true;
    assert!(project(&block(vec![orca])).unwrap().pools.is_empty());
}
#[test]
fn truncated_missing_or_forged_log_evidence_does_not_admit() {
    for alteration in 0..4 {
        let mut input = block(vec![orca_call(50)]);
        let logs = &mut input.transactions[0].meta.as_option_mut().unwrap().log_messages;
        match alteration {
            0 => logs.push("Log truncated".into()),
            1 => {
                logs.remove(0);
            }
            2 => {
                logs[0] = format!("Program {} invoke [2]", base58::encode(orca::PROGRAM_ID));
            }
            _ => {
                logs[2] = format!("Program {} success", base58::encode([88; 32]));
            }
        }
        assert!(project(&input).unwrap().pools.is_empty());
    }
    let mut call = orca_call(50);
    let logs = std::mem::take(&mut call.logs);
    let mut child = Call::new([88; 32], vec![], vec![]);
    child.logs = logs;
    call.children.push(child);
    assert!(project(&block(vec![call])).unwrap().pools.is_empty());
}
#[test]
fn initialization_mismatch_or_stale_event_is_rejected() {
    let mut mismatched = orca_call(50);
    mismatched.accounts[6] = [60; 32];
    let mut unrelated = Call::new(orca::PROGRAM_ID, vec![], vec![0; 8]);
    unrelated.logs = orca_call(51).logs;
    assert!(project(&block(vec![mismatched, unrelated])).unwrap().pools.is_empty());
    let mut parent = pump_call(50);
    parent.accounts[3] = [60; 32];
    assert!(project(&block(vec![parent])).unwrap().pools.is_empty());
}
#[test]
fn event_self_cpi_must_belong_to_its_initialization() {
    let mut call = pump_call(50);
    let event = call.children.remove(0);
    assert!(project(&block(vec![call, event])).unwrap().pools.is_empty());
    let mut call = pump_call(50);
    let event = call.children.remove(0);
    let mut middle = Call::new([88; 32], vec![], vec![]);
    middle.children.push(event);
    call.children.push(middle);
    assert!(project(&block(vec![call])).unwrap().pools.is_empty());
}
#[test]
fn duplicates_are_stable_and_conflicts_reject_whole_block() {
    let input = block(vec![orca_call(50), orca_call(50)]);
    let first = project(&input).unwrap();
    assert_eq!(first.pools.len(), 1);
    assert_eq!(first, project(&input).unwrap());
    assert_eq!(first.pools[0].creation.instruction.instruction_index, 0);
    let mut conflict = orca_call(50);
    conflict.accounts[7] = [77; 32];
    assert!(matches!(
        project(&block(vec![orca_call(50), conflict])),
        Err(ProjectError::ConflictingPool { .. })
    ));
}
#[test]
fn all_resource_limits_are_enforced_before_projection() {
    let input = block(vec![orca_call(50)]);
    for limits in [
        Limits {
            max_transactions: 0,
            ..Limits::default()
        },
        Limits {
            max_instructions: 0,
            ..Limits::default()
        },
        Limits {
            max_log_messages: 0,
            ..Limits::default()
        },
        Limits {
            max_log_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_pools: 0,
            ..Limits::default()
        },
        Limits {
            max_input_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_account_entries: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(project_with_limits(&input, limits), Err(ProjectError::Capacity)));
    }
}
#[test]
fn invalid_and_unsupported_inputs_fail_closed() {
    let mut input = block(vec![orca_call(50)]);
    input.transactions[0]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .instructions[0]
        .program_id_index = 999;
    assert!(matches!(project(&input), Err(ProjectError::InvalidBlock(_))));
    let mut input = block(vec![orca_call(50)]);
    input.transactions[0]
        .transaction
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .version = Some(42);
    assert!(matches!(project(&input), Err(ProjectError::UnsupportedMessage(_))));
}
#[test]
fn pool_order_is_deterministic_and_transaction_index_refers_to_supplied_input() {
    let mut input = block(vec![orca_call(70)]);
    let empty = block(vec![]).transactions.remove(0);
    input.transactions.insert(0, empty);
    assert_eq!(project(&input).unwrap().pools[0].creation.transaction_index, 1);
    assert_eq!(
        project(&block(vec![orca_call(70), orca_call(50)]))
            .unwrap()
            .pools
            .iter()
            .map(|p| p.address[0])
            .collect::<Vec<_>>(),
        vec![50, 70]
    );
}
#[test]
fn legacy_pump_v1_is_explicitly_not_covered() {
    let mut call = pump_call(50);
    call.data = tagged(
        "global:create_pool",
        &pump::instructions::CreatePoolInstructionV1 {
            index: 1,
            base_amount_in: 10,
            quote_amount_in: 20,
        },
    );
    assert!(project(&block(vec![call])).unwrap().pools.is_empty());
}

#[test]
fn orca_legacy_and_adaptive_initialization_use_their_own_layouts() {
    let mut legacy = orca_call(50);
    legacy.accounts = vec![
        [1; 32],
        [2; 32],
        [3; 32],
        [6; 32],
        [50; 32],
        [8; 32],
        [9; 32],
        [10; 32],
        token::PROGRAM_ID,
        [13; 32],
        [14; 32],
    ];
    legacy.data = tagged(
        "global:initialize_pool",
        &orca::instructions::InitializePoolInstruction {
            bumps: orca::instructions::WhirlpoolBumps { whirlpool_bump: 1 },
            tick_spacing: 64,
            initial_sqrt_price: 1u128 << 64,
        },
    );
    let legacy_pool = project(&block(vec![legacy])).unwrap().pools.remove(0);
    assert_eq!(legacy_pool.tokens[0].vault, Some([8; 32]));
    let mut adaptive = orca_call(51);
    adaptive.accounts = vec![
        [1; 32],
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [6; 32],
        [20; 32],
        [51; 32],
        [21; 32],
        [8; 32],
        [9; 32],
        [10; 32],
        token::PROGRAM_ID,
        token::PROGRAM_ID,
        [13; 32],
        [14; 32],
    ];
    adaptive.data = tagged(
        "global:initialize_pool_with_adaptive_fee",
        &orca::instructions::InitializePoolWithAdaptiveFeeInstruction {
            initial_sqrt_price: 1u128 << 64,
            trade_enable_timestamp: None,
        },
    );
    let adaptive_pool = project(&block(vec![adaptive])).unwrap().pools.remove(0);
    assert_eq!(adaptive_pool.tokens, legacy_pool.tokens);
}

#[test]
fn raw_decoder_rejects_bad_program_keys_mismatches_and_oversized_data() {
    let call = orca_call(50);
    let event = STANDARD.decode(call.logs[0].strip_prefix("Program data: ").unwrap()).unwrap();
    let accounts: Vec<_> = call.accounts.iter().map(|key| key.as_slice()).collect();
    let candidate = dex_pools::decode_pool_creation(&call.program, &call.data, &accounts, &event).unwrap();
    assert_eq!(candidate.address, [50; 32]);
    assert!(dex_pools::decode_pool_creation(&[7; 32], &call.data, &accounts, &event).is_none());
    assert!(dex_pools::decode_pool_creation(&call.program, &vec![0; 2049], &accounts, &event).is_none());
    assert!(dex_pools::decode_pool_creation(&call.program, &call.data, &accounts, &vec![0; 2049]).is_none());
    let mut bad = accounts.clone();
    bad[6] = &[3; 31];
    assert!(dex_pools::decode_pool_creation(&call.program, &call.data, &bad, &event).is_none());
    let mut oversized = pump_call(50);
    oversized.children[0].data.resize(2049, 0);
    assert!(project(&block(vec![oversized])).unwrap().pools.is_empty());
}

#[test]
fn unresolved_programs_or_noncanonical_tokens_are_not_invented() {
    let mut call = orca_call(50);
    call.accounts[10] = [33; 32];
    assert!(project(&block(vec![call])).unwrap().pools.is_empty());
    let mut call = orca_call(50);
    call.logs.clear();
    assert!(project(&block(vec![call])).unwrap().pools.is_empty());
    let mut call = pump_call(50);
    call.children[0].failed = true;
    assert!(project(&block(vec![call])).unwrap().pools.is_empty());
}

#[test]
fn each_orca_event_identity_field_must_match_the_initialization() {
    let call = orca_call(50);
    let raw = STANDARD.decode(call.logs[0].strip_prefix("Program data: ").unwrap()).unwrap();
    let orca::events::WhirlpoolEvent::PoolInitialized(original) = orca::events::parse_event(&raw).unwrap() else {
        panic!("fixture")
    };
    let accounts: Vec<_> = call.accounts.iter().map(|key| key.as_slice()).collect();
    for field in 0..9 {
        let mut event = original.clone();
        match field {
            0 => event.whirlpool = key(99),
            1 => event.whirlpools_config = key(99),
            2 => event.token_mint_a = key(99),
            3 => event.token_mint_b = key(99),
            4 => event.token_program_a = key(99),
            5 => event.token_program_b = key(99),
            6 => event.tick_spacing += 1,
            7 => event.initial_sqrt_price += 1,
            _ => event.token_mint_b = event.token_mint_a,
        }
        assert!(
            dex_pools::decode_pool_creation(&call.program, &call.data, &accounts, &tagged("event:PoolInitialized", &event)).is_none(),
            "field {field}"
        );
    }
}

#[test]
fn each_pump_event_identity_field_must_match_the_initialization() {
    let call = pump_call(50);
    let pump::events::PumpFunAmmEvent::CreatePoolEventV2(original) = pump::events::unpack(&call.children[0].data).unwrap() else {
        panic!("fixture")
    };
    let accounts: Vec<_> = call.accounts.iter().map(|key| key.as_slice()).collect();
    for field in 0..11 {
        let mut event = original.clone();
        match field {
            0 => event.pool = key(99),
            1 => event.base_mint = key(99),
            2 => event.quote_mint = key(99),
            3 => event.creator = key(99),
            4 => event.lp_mint = key(99),
            5 => event.user_base_token_account = key(99),
            6 => event.user_quote_token_account = key(99),
            7 => event.index += 1,
            8 => event.base_amount_in += 1,
            9 => event.quote_amount_in += 1,
            _ => event.coin_creator = key(99),
        }
        let mut raw = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
        raw.extend(tagged("event:CreatePoolEvent", &event));
        assert!(
            dex_pools::decode_pool_creation(&call.program, &call.data, &accounts, &raw).is_none(),
            "field {field}"
        );
    }
}

fn current_pump_call(pool: u8) -> Call {
    let mut call = pump_call(pool);
    let pump::instructions::PumpFunAmmInstruction::CreatePoolV2(base) = pump::instructions::unpack(&call.data).unwrap() else {
        panic!("fixture")
    };
    let init = pump::instructions::CreatePoolInstructionV3 {
        base,
        is_mayhem_mode: false,
        is_cashback_coin: pump::instructions::OptionBool(false),
        creator_fee_bps: pump::instructions::OptionU64(100),
        can_edit_creator_fee: pump::instructions::OptionBool(false),
        is_holder_reward: pump::instructions::OptionBool(true),
    };
    call.data = tagged("global:create_pool", &init);
    let pump::events::PumpFunAmmEvent::CreatePoolEventV2(base) = pump::events::unpack(&call.children[0].data).unwrap() else {
        panic!("fixture")
    };
    let event = pump::events::CreatePoolEventV3 {
        base,
        is_mayhem_mode: false,
        creator_fee_bps: 100,
        can_edit_creator_fee: false,
        is_holder_reward: true,
    };
    let mut raw = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
    raw.extend(tagged("event:CreatePoolEvent", &event));
    call.children[0].data = raw;
    call
}

#[test]
fn current_pump_creation_uses_shared_v3_types_and_verified_ancestry() {
    let call = current_pump_call(50);
    let output = project(&block(vec![call])).unwrap();
    assert_eq!(output.pools.len(), 1);
    assert_eq!(output.pools[0].protocol, Protocol::PumpfunAmm);
    assert_eq!(output.pools[0].tokens, project(&block(vec![pump_call(50)])).unwrap().pools[0].tokens);
    let mut failed = current_pump_call(50);
    failed.failed = true;
    assert!(project(&block(vec![failed])).unwrap().pools.is_empty());
}

#[test]
fn current_pump_suffix_mismatch_and_mixed_versions_are_rejected() {
    for field in 0..4 {
        let mut call = current_pump_call(50);
        let pump::events::PumpFunAmmEvent::CreatePoolEventV3(mut event) = pump::events::unpack(&call.children[0].data).unwrap() else {
            panic!("fixture")
        };
        match field {
            0 => event.is_mayhem_mode = true,
            1 => event.creator_fee_bps += 1,
            2 => event.can_edit_creator_fee = true,
            _ => event.is_holder_reward = false,
        }
        let mut raw = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
        raw.extend(tagged("event:CreatePoolEvent", &event));
        call.children[0].data = raw;
        assert!(project(&block(vec![call])).unwrap().pools.is_empty(), "field {field}");
    }
    let mut current = current_pump_call(50);
    current.children[0].data = pump_call(50).children.remove(0).data;
    assert!(project(&block(vec![current])).unwrap().pools.is_empty());
    let mut old = pump_call(50);
    old.children[0].data = current_pump_call(50).children.remove(0).data;
    assert!(project(&block(vec![old])).unwrap().pools.is_empty());
}
