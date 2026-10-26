//! Synthetic initialization fixtures: argument encodings use shared IDL types,
//! and named account lists below follow the bundled IDL JSON at revision
//! 0e7f1d0e303fb7592353a2abe892000394ba4be6. The additional CLMM account lists
//! follow official SDK73e95fc and program edfc5c0, documented in the shared IDL.
//! These are not mainnet captures.
use borsh::{BorshDeserialize, BorshSerialize};
use dex_pools::{
    decode_pool_initialization, project,
    solana::{base58, pb::sf::solana::r#type::v1 as pb},
    Candidate, EvidencePosition, Protocol,
};
use substreams_solana_idls::{
    meteora::{amm, daam, dlmm},
    raydium::{amm::v4 as amm_v4, clmm::v3 as clmm, cpmm},
    spl::{token, token_2022},
};

#[derive(Clone)]
struct Fixture {
    name: &'static str,
    program: [u8; 32],
    protocol: Protocol,
    data: Vec<u8>,
    roles: Vec<&'static str>,
    accounts: Vec<[u8; 32]>,
    pool: &'static str,
    mints: [&'static str; 2],
    vaults: [Option<&'static str>; 2],
    programs: [&'static str; 2],
}

impl Fixture {
    fn index(&self, role: &str) -> usize {
        self.roles
            .iter()
            .position(|name| *name == role)
            .unwrap_or_else(|| panic!("{}: missing role {role}", self.name))
    }
    fn key(&self, role: &str) -> [u8; 32] {
        self.accounts[self.index(role)]
    }
    fn decode(&self) -> Option<Candidate> {
        let accounts: Vec<_> = self.accounts.iter().map(|key| key.as_slice()).collect();
        decode_pool_initialization(&self.program, &self.data, &accounts)
    }
}

// AMM-v4 argument fields are private upstream. Reading zero values through the
// shared Borsh type, then serializing that type, avoids a local payload layout.
// All initializer argument types here have bounded fields and valid zero enum
// discriminants. Admission tests are synthetic; they do not prove on-chain PDA
// constraints or that a zero-liquidity initialization would execute on-chain.
fn payload<T: BorshDeserialize + BorshSerialize>(discriminator: &[u8]) -> Vec<u8> {
    let args = T::deserialize_reader(&mut std::io::repeat(0)).unwrap();
    let mut data = discriminator.to_vec();
    data.extend(borsh::to_vec(&args).unwrap());
    data
}

fn fixture(
    name: &'static str,
    program: [u8; 32],
    protocol: Protocol,
    data: Vec<u8>,
    roles: &'static str,
    pool: &'static str,
    mints: [&'static str; 2],
    vaults: [Option<&'static str>; 2],
    programs: [&'static str; 2],
) -> Fixture {
    let roles: Vec<_> = roles.split_whitespace().collect();
    let accounts = roles
        .iter()
        .enumerate()
        .map(|(i, name)| match *name {
            "tokenProgram" | "token_program" | "token_program_0" | "token_0_program" | "token_program_x" | "token_a_program" => token::PROGRAM_ID,
            "token_program_1" | "token_1_program" | "token_program_y" | "token_b_program" | "token_2022_program" => token_2022::PROGRAM_ID,
            "program" => program,
            _ => [i as u8 + 10; 32],
        })
        .collect();
    Fixture {
        name,
        program,
        protocol,
        data,
        roles,
        accounts,
        pool,
        mints,
        vaults,
        programs,
    }
}

fn fixtures() -> Vec<Fixture> {
    vec![
        // raydium/amm/v4/idl.json: initialize.
        fixture("amm_v4/initialize", amm_v4::PROGRAM_ID, Protocol::RaydiumAmmV4,
            payload::<amm_v4::instructions::InitializeInstruction>(&[amm_v4::instructions::INITIALIZE]),
            "tokenProgram systemProgram rent amm ammAuthority ammOpenOrders lpMintAddress coinMintAddress pcMintAddress poolCoinTokenAccount poolPcTokenAccount poolWithdrawQueue poolTargetOrdersAccount userLpTokenAccount poolTempLpTokenAccount serumProgram serumMarket userWallet",
            "amm", ["coinMintAddress", "pcMintAddress"],
            [Some("poolCoinTokenAccount"), Some("poolPcTokenAccount")], ["tokenProgram", "tokenProgram"]),
        // raydium/amm/v4/idl.json: initialize2.
        fixture("amm_v4/initialize2", amm_v4::PROGRAM_ID, Protocol::RaydiumAmmV4,
            payload::<amm_v4::instructions::Initialize2Instruction>(&[amm_v4::instructions::INITIALIZE2]),
            "tokenProgram splAssociatedTokenAccount systemProgram rent amm ammAuthority ammOpenOrders lpMint coinMint pcMint poolCoinTokenAccount poolPcTokenAccount poolWithdrawQueue ammTargetOrders poolTempLp serumProgram serumMarket userWallet userTokenCoin userTokenPc userLpTokenAccount",
            "amm", ["coinMint", "pcMint"],
            [Some("poolCoinTokenAccount"), Some("poolPcTokenAccount")], ["tokenProgram", "tokenProgram"]),
        // raydium/clmm/v3/idl.json: create_pool.
        fixture("clmm/create_pool", clmm::PROGRAM_ID, Protocol::RaydiumClmm,
            payload::<clmm::instructions::CreatePoolInstruction>(&clmm::instructions::CREATE_POOL),
            "pool_creator amm_config pool_state token_mint_0 token_mint_1 token_vault_0 token_vault_1 observation_state tick_array_bitmap token_program_0 token_program_1 system_program rent",
            "pool_state", ["token_mint_0", "token_mint_1"],
            [Some("token_vault_0"), Some("token_vault_1")], ["token_program_0", "token_program_1"]),
        // Current SDK73e95fc / historical program edfc5c0; see shared IDL docs.
        fixture("clmm/create_customizable_pool", clmm::PROGRAM_ID, Protocol::RaydiumClmm,
            payload::<clmm::instructions::CreateCustomizablePoolInstruction>(&clmm::instructions::CREATE_CUSTOMIZABLE_POOL),
            "pool_creator amm_config pool_state token_mint_0 token_mint_1 token_vault_0 token_vault_1 observation_state tick_array_bitmap token_program_0 token_program_1 system_program rent", "pool_state", ["token_mint_0", "token_mint_1"],
            [Some("token_vault_0"), Some("token_vault_1")], ["token_program_0", "token_program_1"]),
        // Current SDK73e95fc / historical program edfc5c0; see shared IDL docs.
        fixture("clmm/create_customizable_pool_v2", clmm::PROGRAM_ID, Protocol::RaydiumClmm,
            payload::<clmm::instructions::CreateCustomizablePoolV2Instruction>(&clmm::instructions::CREATE_CUSTOMIZABLE_POOL_V2),
            "payer pool_creator amm_config pool_state token_mint_0 token_mint_1 token_vault_0 token_vault_1 observation_state tick_array_bitmap token_program_0 token_program_1 system_program rent", "pool_state", ["token_mint_0", "token_mint_1"],
            [Some("token_vault_0"), Some("token_vault_1")], ["token_program_0", "token_program_1"]),
        // Current SDK73e95fc / historical program edfc5c0; see shared IDL docs.
        fixture("clmm/create_permissioned_pool", clmm::PROGRAM_ID, Protocol::RaydiumClmm,
            payload::<clmm::instructions::CreatePermissionedPoolInstruction>(&clmm::instructions::CREATE_PERMISSIONED_POOL),
            "payer pool_creator permission amm_config pool_state token_mint_0 token_mint_1 token_vault_0 token_vault_1 observation_state tick_array_bitmap token_program_0 token_program_1 system_program rent", "pool_state", ["token_mint_0", "token_mint_1"],
            [Some("token_vault_0"), Some("token_vault_1")], ["token_program_0", "token_program_1"]),
        // raydium/cpmm/idl-v2.json: initialize.
        fixture("cpmm/initialize", cpmm::PROGRAM_ID, Protocol::RaydiumCpmm,
            payload::<cpmm::instructions::InitializeInstruction>(&cpmm::instructions::INITIALIZE),
            "creator amm_config authority pool_state token_0_mint token_1_mint lp_mint creator_token_0 creator_token_1 creator_lp_token token_0_vault token_1_vault create_pool_fee observation_state token_program token_0_program token_1_program associated_token_program system_program rent",
            "pool_state", ["token_0_mint", "token_1_mint"],
            [Some("token_0_vault"), Some("token_1_vault")], ["token_0_program", "token_1_program"]),
        // raydium/cpmm/idl-v2.json: initialize_with_permission.
        fixture("cpmm/initialize_with_permission", cpmm::PROGRAM_ID, Protocol::RaydiumCpmm,
            payload::<cpmm::instructions::InitializeWithPermissionInstruction>(&cpmm::instructions::INITIALIZE_WITH_PERMISSION),
            "payer creator amm_config authority pool_state token_0_mint token_1_mint lp_mint payer_token_0 payer_token_1 payer_lp_token token_0_vault token_1_vault create_pool_fee observation_state permission token_program token_0_program token_1_program associated_token_program system_program",
            "pool_state", ["token_0_mint", "token_1_mint"],
            [Some("token_0_vault"), Some("token_1_vault")], ["token_0_program", "token_1_program"]),
        // meteora/amm/idl.json: initializePermissionedPool.
        fixture("amm/initializePermissionedPool", amm::PROGRAM_ID, Protocol::MeteoraAmm,
            payload::<amm::instructions::InitializePermissionedPoolInstruction>(&amm::instructions::INITIALIZE_PERMISSIONED_POOL),
            "pool lpMint tokenAMint tokenBMint aVault bVault aVaultLpMint bVaultLpMint aVaultLp bVaultLp adminTokenA adminTokenB adminPoolLp protocolTokenAFee protocolTokenBFee admin feeOwner rent mintMetadata metadataProgram vaultProgram tokenProgram associatedTokenProgram systemProgram",
            "pool", ["tokenAMint", "tokenBMint"],
            [None, None], ["tokenProgram", "tokenProgram"]),
        // meteora/amm/idl.json: initializePermissionlessPool.
        fixture("amm/initializePermissionlessPool", amm::PROGRAM_ID, Protocol::MeteoraAmm,
            payload::<amm::instructions::InitializePermissionlessPoolInstruction>(&amm::instructions::INITIALIZE_PERMISSIONLESS_POOL),
            "pool lpMint tokenAMint tokenBMint aVault bVault aTokenVault bTokenVault aVaultLpMint bVaultLpMint aVaultLp bVaultLp payerTokenA payerTokenB payerPoolLp protocolTokenAFee protocolTokenBFee payer feeOwner rent mintMetadata metadataProgram vaultProgram tokenProgram associatedTokenProgram systemProgram",
            "pool", ["tokenAMint", "tokenBMint"],
            [Some("aTokenVault"), Some("bTokenVault")], ["tokenProgram", "tokenProgram"]),
        // meteora/amm/idl.json: initializePermissionlessPoolWithFeeTier.
        fixture("amm/initializePermissionlessPoolWithFeeTier", amm::PROGRAM_ID, Protocol::MeteoraAmm,
            payload::<amm::instructions::InitializePermissionlessPoolWithFeeTierInstruction>(&amm::instructions::INITIALIZE_PERMISSIONLESS_POOL_WITH_FEE_TIER),
            "pool lpMint tokenAMint tokenBMint aVault bVault aTokenVault bTokenVault aVaultLpMint bVaultLpMint aVaultLp bVaultLp payerTokenA payerTokenB payerPoolLp protocolTokenAFee protocolTokenBFee payer feeOwner rent mintMetadata metadataProgram vaultProgram tokenProgram associatedTokenProgram systemProgram",
            "pool", ["tokenAMint", "tokenBMint"],
            [Some("aTokenVault"), Some("bTokenVault")], ["tokenProgram", "tokenProgram"]),
        // meteora/amm/idl.json: initializePermissionlessConstantProductPoolWithConfig.
        fixture("amm/initializePermissionlessConstantProductPoolWithConfig", amm::PROGRAM_ID, Protocol::MeteoraAmm,
            payload::<amm::instructions::InitializePermissionlessConstantProductPoolWithConfigInstruction>(&amm::instructions::INITIALIZE_PERMISSIONLESS_CONSTANT_PRODUCT_POOL_WITH_CONFIG),
            "pool config lpMint tokenAMint tokenBMint aVault bVault aTokenVault bTokenVault aVaultLpMint bVaultLpMint aVaultLp bVaultLp payerTokenA payerTokenB payerPoolLp protocolTokenAFee protocolTokenBFee payer rent mintMetadata metadataProgram vaultProgram tokenProgram associatedTokenProgram systemProgram",
            "pool", ["tokenAMint", "tokenBMint"],
            [Some("aTokenVault"), Some("bTokenVault")], ["tokenProgram", "tokenProgram"]),
        // meteora/amm/idl.json: initializePermissionlessConstantProductPoolWithConfig2.
        fixture("amm/initializePermissionlessConstantProductPoolWithConfig2", amm::PROGRAM_ID, Protocol::MeteoraAmm,
            payload::<amm::instructions::InitializePermissionlessConstantProductPoolWithConfig2Instruction>(&amm::instructions::INITIALIZE_PERMISSIONLESS_CONSTANT_PRODUCT_POOL_WITH_CONFIG2),
            "pool config lpMint tokenAMint tokenBMint aVault bVault aTokenVault bTokenVault aVaultLpMint bVaultLpMint aVaultLp bVaultLp payerTokenA payerTokenB payerPoolLp protocolTokenAFee protocolTokenBFee payer rent mintMetadata metadataProgram vaultProgram tokenProgram associatedTokenProgram systemProgram",
            "pool", ["tokenAMint", "tokenBMint"],
            [Some("aTokenVault"), Some("bTokenVault")], ["tokenProgram", "tokenProgram"]),
        // meteora/amm/idl.json: initializeCustomizablePermissionlessConstantProductPool.
        fixture("amm/initializeCustomizablePermissionlessConstantProductPool", amm::PROGRAM_ID, Protocol::MeteoraAmm,
            payload::<amm::instructions::InitializeCustomizablePermissionlessConstantProductPoolInstruction>(&amm::instructions::INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_CONSTANT_PRODUCT_POOL),
            "pool lpMint tokenAMint tokenBMint aVault bVault aTokenVault bTokenVault aVaultLpMint bVaultLpMint aVaultLp bVaultLp payerTokenA payerTokenB payerPoolLp protocolTokenAFee protocolTokenBFee payer rent mintMetadata metadataProgram vaultProgram tokenProgram associatedTokenProgram systemProgram",
            "pool", ["tokenAMint", "tokenBMint"],
            [Some("aTokenVault"), Some("bTokenVault")], ["tokenProgram", "tokenProgram"]),
        // meteora/dlmm/idl.json: initialize_customizable_permissionless_lb_pair.
        fixture("dlmm/initialize_customizable_permissionless_lb_pair", dlmm::PROGRAM_ID, Protocol::MeteoraDlmm,
            payload::<dlmm::instructions::InitializeCustomizablePermissionlessLbPairInstruction>(&dlmm::instructions::INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR),
            "lb_pair bin_array_bitmap_extension token_mint_x token_mint_y reserve_x reserve_y oracle user_token_x funder token_program system_program user_token_y event_authority program",
            "lb_pair", ["token_mint_x", "token_mint_y"],
            [Some("reserve_x"), Some("reserve_y")], ["token_program", "token_program"]),
        // meteora/dlmm/idl.json: initialize_customizable_permissionless_lb_pair2.
        fixture("dlmm/initialize_customizable_permissionless_lb_pair2", dlmm::PROGRAM_ID, Protocol::MeteoraDlmm,
            payload::<dlmm::instructions::InitializeCustomizablePermissionlessLbPair2Instruction>(&dlmm::instructions::INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR2),
            "lb_pair bin_array_bitmap_extension token_mint_x token_mint_y reserve_x reserve_y oracle user_token_x funder token_badge_x token_badge_y token_program_x token_program_y system_program user_token_y event_authority program",
            "lb_pair", ["token_mint_x", "token_mint_y"],
            [Some("reserve_x"), Some("reserve_y")], ["token_program_x", "token_program_y"]),
        // meteora/dlmm/idl.json: initialize_lb_pair.
        fixture("dlmm/initialize_lb_pair", dlmm::PROGRAM_ID, Protocol::MeteoraDlmm,
            payload::<dlmm::instructions::InitializeLbPairInstruction>(&dlmm::instructions::INITIALIZE_LB_PAIR),
            "lb_pair bin_array_bitmap_extension token_mint_x token_mint_y reserve_x reserve_y oracle preset_parameter funder token_program system_program rent event_authority program",
            "lb_pair", ["token_mint_x", "token_mint_y"],
            [Some("reserve_x"), Some("reserve_y")], ["token_program", "token_program"]),
        // meteora/dlmm/idl.json: initialize_lb_pair2.
        fixture("dlmm/initialize_lb_pair2", dlmm::PROGRAM_ID, Protocol::MeteoraDlmm,
            payload::<dlmm::instructions::InitializeLbPair2Instruction>(&dlmm::instructions::INITIALIZE_LB_PAIR2),
            "lb_pair bin_array_bitmap_extension token_mint_x token_mint_y reserve_x reserve_y oracle preset_parameter funder token_badge_x token_badge_y token_program_x token_program_y system_program event_authority program",
            "lb_pair", ["token_mint_x", "token_mint_y"],
            [Some("reserve_x"), Some("reserve_y")], ["token_program_x", "token_program_y"]),
        // meteora/dlmm/idl.json: initialize_permission_lb_pair.
        fixture("dlmm/initialize_permission_lb_pair", dlmm::PROGRAM_ID, Protocol::MeteoraDlmm,
            payload::<dlmm::instructions::InitializePermissionLbPairInstruction>(&dlmm::instructions::INITIALIZE_PERMISSION_LB_PAIR),
            "base lb_pair bin_array_bitmap_extension token_mint_x token_mint_y reserve_x reserve_y oracle admin token_badge_x token_badge_y token_program_x token_program_y system_program rent event_authority program",
            "lb_pair", ["token_mint_x", "token_mint_y"],
            [Some("reserve_x"), Some("reserve_y")], ["token_program_x", "token_program_y"]),
        // meteora/daam/idl.json: initialize_customizable_pool.
        fixture("daam/initialize_customizable_pool", daam::PROGRAM_ID, Protocol::MeteoraDaam,
            payload::<daam::instructions::InitializeCustomizablePoolInstruction>(&daam::instructions::INITIALIZE_CUSTOMIZABLE_POOL),
            "creator position_nft_mint position_nft_account payer pool_authority pool position token_a_mint token_b_mint token_a_vault token_b_vault payer_token_a payer_token_b token_a_program token_b_program token_2022_program system_program event_authority program",
            "pool", ["token_a_mint", "token_b_mint"],
            [Some("token_a_vault"), Some("token_b_vault")], ["token_a_program", "token_b_program"]),
        // meteora/daam/idl.json: initialize_pool.
        fixture("daam/initialize_pool", daam::PROGRAM_ID, Protocol::MeteoraDaam,
            payload::<daam::instructions::InitializePoolInstruction>(&daam::instructions::INITIALIZE_POOL),
            "creator position_nft_mint position_nft_account payer config pool_authority pool position token_a_mint token_b_mint token_a_vault token_b_vault payer_token_a payer_token_b token_a_program token_b_program token_2022_program system_program event_authority program",
            "pool", ["token_a_mint", "token_b_mint"],
            [Some("token_a_vault"), Some("token_b_vault")], ["token_a_program", "token_b_program"]),
        // meteora/daam/idl.json: initialize_pool_with_dynamic_config.
        fixture("daam/initialize_pool_with_dynamic_config", daam::PROGRAM_ID, Protocol::MeteoraDaam,
            payload::<daam::instructions::InitializePoolWithDynamicConfigInstruction>(&daam::instructions::INITIALIZE_POOL_WITH_DYNAMIC_CONFIG),
            "creator position_nft_mint position_nft_account payer pool_creator_authority config pool_authority pool position token_a_mint token_b_mint token_a_vault token_b_vault payer_token_a payer_token_b token_a_program token_b_program token_2022_program system_program event_authority program",
            "pool", ["token_a_mint", "token_b_mint"],
            [Some("token_a_vault"), Some("token_b_vault")], ["token_a_program", "token_b_program"]),
    ]
}

fn block(f: &Fixture) -> pb::Block {
    let mut keys = vec![vec![254; 32], f.program.to_vec()];
    let mut account_indexes = vec![];
    for key in &f.accounts {
        let index = match keys.iter().position(|existing| existing.as_slice() == key) {
            Some(index) => index,
            None => {
                keys.push(key.to_vec());
                keys.len() - 1
            }
        };
        account_indexes.push(index as u8);
    }
    let program = base58::encode(f.program);
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
                    instructions: vec![pb::CompiledInstruction {
                        program_id_index: 1,
                        accounts: account_indexes,
                        data: f.data.clone(),
                    }],
                    ..Default::default()
                }
                .into(),
            }
            .into(),
            meta: pb::TransactionStatusMeta {
                log_messages: vec![format!("Program {program} invoke [1]"), format!("Program {program} success")],
                ..Default::default()
            }
            .into(),
        }],
        ..Default::default()
    }
}

#[test]
fn all_twenty_two_initializers_preserve_protocol_order_and_available_identity_fields() {
    let cases = fixtures();
    assert_eq!(cases.len(), 22);
    for f in cases {
        let candidate = f.decode().unwrap_or_else(|| panic!("{} did not decode", f.name));
        assert_eq!(candidate.protocol, f.protocol, "{}", f.name);
        assert_eq!(candidate.program_id, f.program, "{}", f.name);
        assert_eq!(candidate.address, f.key(f.pool), "{}", f.name);
        assert_eq!(candidate.tokens.len(), 2, "{}", f.name);
        for i in 0..2 {
            let token = &candidate.tokens[i];
            assert_eq!(token.mint, f.key(f.mints[i]), "{} token{i} mint", f.name);
            assert_eq!(token.vault, f.vaults[i].map(|role| f.key(role)), "{} token{i} vault", f.name);
            assert_eq!(token.token_program, Some(f.key(f.programs[i])), "{} token{i} program", f.name);
            assert_eq!(token.decimals, None, "{} has no mint decimal evidence", f.name);
            assert!(!token.is_native, "{} is SPL", f.name);
        }
        let mut reversed = f.clone();
        let a = reversed.index(f.mints[0]);
        let b = reversed.index(f.mints[1]);
        reversed.accounts[a] = [250; 32];
        reversed.accounts[b] = [1; 32];
        let candidate = reversed.decode().unwrap();
        assert_eq!(candidate.tokens[0].mint, [250; 32], "{} must not sort token roles", f.name);
        assert_eq!(candidate.tokens[1].mint, [1; 32], "{} must not sort token roles", f.name);
    }
}

#[test]
fn every_initializer_rejects_truncated_extended_wrong_program_and_incomplete_accounts() {
    for f in fixtures() {
        let accounts: Vec<_> = f.accounts.iter().map(|key| key.as_slice()).collect();
        for length in 0..f.data.len() {
            assert!(
                decode_pool_initialization(&f.program, &f.data[..length], &accounts).is_none(),
                "{} truncated at {length}",
                f.name
            );
        }
        let mut extra = f.data.clone();
        extra.push(0);
        assert!(decode_pool_initialization(&f.program, &extra, &accounts).is_none(), "{} extra payload", f.name);
        for length in 0..accounts.len() {
            assert!(
                decode_pool_initialization(&f.program, &f.data, &accounts[..length]).is_none(),
                "{} only {length} accounts",
                f.name
            );
        }
        assert!(
            decode_pool_initialization(&[255; 32], &f.data, &accounts).is_none(),
            "{} foreign program",
            f.name
        );
        let mut malformed = accounts.clone();
        malformed[0] = &f.accounts[0][..31];
        assert!(
            decode_pool_initialization(&f.program, &f.data, &malformed).is_none(),
            "{} malformed account",
            f.name
        );
    }
}

#[test]
fn invalid_identity_and_event_program_accounts_are_not_candidates() {
    for f in fixtures() {
        let mut bad = f.clone();
        let index = bad.index(f.pool);
        bad.accounts[index] = [0; 32];
        assert!(bad.decode().is_none(), "{} zero pool", f.name);
        let mut bad = f.clone();
        let index = bad.index(f.mints[1]);
        bad.accounts[index] = bad.key(f.mints[0]);
        assert!(bad.decode().is_none(), "{} same mint", f.name);
        let mut bad = f.clone();
        let index = bad.index(f.programs[0]);
        bad.accounts[index] = [252; 32];
        assert!(bad.decode().is_none(), "{} unsupported token program", f.name);
        if let [Some(a), Some(b)] = f.vaults {
            let mut bad = f.clone();
            let index = bad.index(b);
            bad.accounts[index] = bad.key(a);
            assert!(bad.decode().is_none(), "{} same vault", f.name);
        }
        if f.roles.contains(&"program") {
            let mut bad = f.clone();
            let index = bad.index("program");
            bad.accounts[index] = [251; 32];
            assert!(bad.decode().is_none(), "{} wrong event program", f.name);
        }
    }
}

#[test]
fn unrelated_short_raydium_swaps_never_enter_the_shared_swap_parser() {
    for discriminator in [
        amm_v4::instructions::SWAP_BASE_IN,
        amm_v4::instructions::SWAP_BASE_OUT,
        amm_v4::instructions::SWAP_BASE_IN_V2,
        amm_v4::instructions::SWAP_BASE_OUT_V2,
    ] {
        assert!(decode_pool_initialization(&amm_v4::PROGRAM_ID, &[discriminator], &[]).is_none());
    }
}

#[test]
fn all_initializers_require_success_and_expose_honest_initialization_evidence() {
    for f in fixtures() {
        let candidate = f.decode().unwrap();
        let success = block(&f);
        let projected = project(&success).unwrap();
        assert_eq!(projected.pools.len(), 1, "{} success", f.name);
        let pool = &projected.pools[0];
        assert_eq!(pool.protocol, candidate.protocol, "{}", f.name);
        assert_eq!(pool.address, candidate.address, "{}", f.name);
        assert_eq!(pool.tokens, candidate.tokens, "{}", f.name);
        assert_eq!(pool.creation.evidence, EvidencePosition::Initialization, "{}", f.name);
        assert_eq!(pool.creation.instruction.instruction_index, 0);
        assert_eq!(pool.creation.instruction.inner_instruction_index, None);
        let mut failed = success.clone();
        failed.transactions[0].meta.as_option_mut().unwrap().err = pb::TransactionError::default().into();
        assert!(project(&failed).unwrap().pools.is_empty(), "{} failed transaction", f.name);
        let mut unproved = success.clone();
        unproved.transactions[0].meta.as_option_mut().unwrap().log_messages.pop();
        assert!(project(&unproved).unwrap().pools.is_empty(), "{} unclosed invocation", f.name);
        let mut truncated = success;
        truncated.transactions[0]
            .meta
            .as_option_mut()
            .unwrap()
            .log_messages
            .push("Log truncated".into());
        assert!(project(&truncated).unwrap().pools.is_empty(), "{} truncated trace", f.name);
    }
}
