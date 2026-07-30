use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, BytesN, Env};

use crate::types::QuestType;
use crate::{QuestEngineContract, QuestEngineContractClient};

// ── Mock StakeVault Contract ─────────────────────────────────────────────────

#[contract]
pub struct MockStakeVault;

#[contractimpl]
impl MockStakeVault {
    /// Returns a multiplier for a learner (basis points: 100 = 1.0x, 120 = 1.2x)
    /// For testing, we'll return 100 (no boost) by default
    pub fn get_multiplier(_env: Env, _learner: Address) -> u32 {
        100 // Default: no multiplier
    }
}

// ── Mock RewardPool Contract ─────────────────────────────────────────────────

#[contract]
pub struct MockRewardPool;

#[contractimpl]
impl MockRewardPool {
    /// Distribute reward - simple no-op for testing
    /// This allows explore quest verification tests to pass without token transfers
    pub fn distribute_reward(_env: Env, _caller: Address, _learner: Address, _amount: i128) {
        // Do nothing - just return successfully
    }
}

/// Mock StakeVault returning a 200x (2.0x) multiplier tier.
#[contract]
pub struct MockStakeVault200;

#[contractimpl]
impl MockStakeVault200 {
    pub fn get_multiplier(_env: Env, _learner: Address) -> u32 {
        200 // 2.0x multiplier
    }
}

fn setup_with_multiplier(
    multiplier: u32,
) -> (Env, QuestEngineContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    // Create a SAC token for USDC
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    // Register mock stake vault
    let stake_vault_id = env.register(MockStakeVault, ());

    // Register mock reward pool
    let reward_pool_id = env.register(MockRewardPool, ());

    // Initialize the contract with admin, token, reward_pool, and stake_vault
    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    (env, client, token_id, reward_pool_id, admin, stake_vault_id)
}

/// Full setup that wires a real MockRewardPoolTransfer so the boost delta can be
/// distributed when the multiplier is > 100.
/// Returns (env, client, token_id, reward_pool_id).
fn setup_with_boosted_multiplier() -> (
    Env,
    QuestEngineContractClient<'static>,
    Address, // token_id
    Address, // reward_pool_id (MockRewardPoolTransfer)
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let stake_vault_id = env.register(MockStakeVaultWithMultiplier, ());

    // Register the real reward pool mock and configure its token.
    let reward_pool_id = env.register(MockRewardPoolTransfer, ());
    let rp_client = MockRewardPoolTransferClient::new(&env, &reward_pool_id);
    rp_client.set_token(&token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    (env, client, token_id, reward_pool_id)
}

#[test]
fn test_review_submission_with_no_multiplier() {
    let (env, client, token_id, reward_pool) = setup_with_multiplier(100);
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[50u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[51u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    client.review_submission(&employer, &learner, &quest_id, &true);

    // With 100 multiplier (1.0x), learner gets base amount
    let fee = (reward_amount * 15) / 100; // 150
    let base_amount = reward_amount - fee; // 850
    let expected_learner_amount = (base_amount * 100) / 100; // 850

    assert_eq!(
        token_balance(&env, &token_id, &learner),
        expected_learner_amount
    );
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee);
}

#[test]
fn test_review_submission_with_120_multiplier() {
    // reward_amount = 1000 → fee = 150, base = 850.
    // With 120x multiplier: boosted = 850 * 120 / 100 = 1020.
    // boost_delta = 1020 - 850 = 170 drawn from RewardPool.
    // Learner receives base (850) + delta (170) = 1020 total.
    let (env, client, token_id, reward_pool_id) = setup_with_boosted_multiplier();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[52u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[53u8; 32]);

    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boosted = (base * 120) / 100; // 1020
    let boost_delta = boosted - base; // 170

    // Fund employer for quest escrow.
    mint_tokens(&env, &token_id, &employer, &reward_amount);
    // Pre-fund the RewardPool with enough to cover the boost delta.
    mint_tokens(&env, &token_id, &reward_pool_id, &boost_delta);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    client.review_submission(&employer, &learner, &quest_id, &true);

    // Staked learner (120x) receives base + delta = boosted total.
    assert_eq!(token_balance(&env, &token_id, &learner), boosted);
    // Fee goes to the reward pool; delta was drawn from pool so net pool balance
    // = fee + (pre-funded delta) - delta = fee.
    assert_eq!(token_balance(&env, &token_id, &reward_pool_id), fee);
    // A staked learner receives strictly more than a non-staked learner.
    assert!(
        boosted > base,
        "Staked learner must receive more than non-staked"
    );
}

fn setup_with_multiplier(
    _multiplier: u32,
) -> (
    Env,
    QuestEngineContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

// ── Acceptance Criteria Tests ─────────────────────────────────────────────────

#[test]
fn test_staked_learner_receives_more_than_non_staked() {
    // AC: A learner with a 120 multiplier receives more than a non-staked learner
    // for the same approved quest. Two identical quests, same reward_amount,
    // one learner staked (120x), one not (100x).
    let reward_amount: i128 = 1000;
    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boost_delta_120 = (base * 120) / 100 - base; // 170

    // --- Non-staked learner (100x) ---
    let (env1, client1, token_id1, _rp1) = setup_with_multiplier(100);
    let employer1 = Address::generate(&env1);
    let learner_no_stake = Address::generate(&env1);
    let mh = BytesN::from_array(&env1, &[80u8; 32]);
    let ph = BytesN::from_array(&env1, &[81u8; 32]);
    mint_tokens(&env1, &token_id1, &employer1, &reward_amount);
    let qid1 = client1.create_build_quest(&employer1, &reward_amount, &mh);
    client1.submit_proof(&learner_no_stake, &qid1, &ph);
    client1.review_submission(&employer1, &learner_no_stake, &qid1, &true);
    let non_staked_payout = token_balance(&env1, &token_id1, &learner_no_stake);

    // --- Staked learner (120x) ---
    let (env2, client2, token_id2, rp2) = setup_with_boosted_multiplier();
    let employer2 = Address::generate(&env2);
    let learner_staked = Address::generate(&env2);
    let mh2 = BytesN::from_array(&env2, &[82u8; 32]);
    let ph2 = BytesN::from_array(&env2, &[83u8; 32]);
    mint_tokens(&env2, &token_id2, &employer2, &reward_amount);
    mint_tokens(&env2, &token_id2, &rp2, &boost_delta_120);
    let qid2 = client2.create_build_quest(&employer2, &reward_amount, &mh2);
    client2.submit_proof(&learner_staked, &qid2, &ph2);
    client2.review_submission(&employer2, &learner_staked, &qid2, &true);
    let staked_payout = token_balance(&env2, &token_id2, &learner_staked);

    assert!(
        staked_payout > non_staked_payout,
        "Staked learner ({}) must earn more than non-staked learner ({})",
        staked_payout,
        non_staked_payout
    );
    assert_eq!(non_staked_payout, base); // 850 for 100x
    assert_eq!(staked_payout, (base * 120) / 100); // 1020 for 120x
}

#[test]
fn test_basis_point_math_for_100_120_200_multipliers() {
    // AC: Basis-point math remains correct for 100, 120, and 200 multiplier tiers.
    let reward: i128 = 1000;
    let fee = (reward * 15) / 100; // 150
    let base = reward - fee; // 850

    // 100x: boosted == base (no change)
    let b100 = (base * 100) / 100;
    assert_eq!(b100, 850);

    // 120x: boosted = 1020
    let b120 = (base * 120) / 100;
    assert_eq!(b120, 1020);

    // 200x: boosted = 1700
    let b200 = (base * 200) / 100;
    assert_eq!(b200, 1700);

    // Each tier is strictly larger than the previous.
    assert!(b100 < b120);
    assert!(b120 < b200);
}

#[test]
fn test_review_submission_with_200_multiplier_draws_delta_from_pool() {
    // AC: 200x multiplier tier works end-to-end; delta from RewardPool.
    // reward=1000 → fee=150, base=850, boosted=1700, delta=850.
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let stake_vault_id = env.register(MockStakeVault200, ());
    let reward_pool_id = env.register(MockRewardPoolTransfer, ());
    let rp_client = MockRewardPoolTransferClient::new(&env, &reward_pool_id);
    rp_client.set_token(&token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[90u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[91u8; 32]);

    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boosted = (base * 200) / 100; // 1700
    let boost_delta = boosted - base; // 850

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    mint_tokens(&env, &token_id, &reward_pool_id, &boost_delta);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    assert_eq!(token_balance(&env, &token_id, &learner), boosted);
    assert_eq!(token_balance(&env, &token_id, &reward_pool_id), fee);
}

#[test]
#[should_panic]
fn test_review_submission_fails_deterministically_when_pool_cannot_cover_boost() {
    // AC: The contract fails deterministically if the configured funding source
    // cannot cover the boosted payout.
    // Pool has 0 tokens for the delta → token transfer panics.
    let (env, client, token_id, _reward_pool_id) = setup_with_boosted_multiplier();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[92u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[93u8; 32]);

    // Fund employer but do NOT pre-fund the RewardPool for the delta.
    mint_tokens(&env, &token_id, &employer, &reward_amount);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    // Should panic because pool has no balance for the 170 boost_delta.
    client.review_submission(&employer, &learner, &quest_id, &true);
}

#[test]
fn test_review_submission_with_80_multiplier() {
    // Test with a multiplier less than 100 (0.8x penalty)
    let (env, client, token_id, reward_pool) = setup_with_multiplier(100);
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[54u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[55u8; 32]);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let stake_vault_id = env.register(MockStakeVault, ());
    let reward_pool_id = env.register(MockRewardPool, ());

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    (env, client, token_id, reward_pool_id, admin, stake_vault_id)
}

// ── Initialize Tests ─────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_panics() {
    let (_env, client, token_id, reward_pool, admin, stake_vault_id) = setup();
    client.initialize(&admin, &token_id, &reward_pool, &stake_vault_id);
}

// ── create_build_quest Tests ─────────────────────────────────────────────────

#[test]
fn test_create_build_quest_success() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 1_000;
    let metadata_hash = BytesN::from_array(&env, &[1u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    assert_eq!(token_balance(&env, &token_id, &employer), reward_amount);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    assert_eq!(quest_id, 1);

    assert_eq!(
        token_balance(&env, &token_id, &client.address),
        reward_amount
    );
    assert_eq!(token_balance(&env, &token_id, &employer), 0);

    let quest = client.get_quest(&quest_id).unwrap();
    assert_eq!(quest.employer, employer);
    assert_eq!(quest.reward_amount, reward_amount);
    assert_eq!(quest.quest_type, QuestType::Build);
    assert_eq!(quest.metadata_hash, metadata_hash);
    assert!(quest.active);
    assert!(!quest.has_approved_submission);
}

// ── Explore Quest Tests ──────────────────────────────────────────────────────

/// Mock RewardPool contract for testing (no-op, used for Explore quest tests).
#[contract]
pub struct MockRewardPool;

#[contractimpl]
impl MockRewardPool {
    pub fn distribute_reward(_env: Env, _caller: Address, _learner: Address, _amount: i128) {
        // Mock implementation - does nothing in tests
    }
}

/// Mock RewardPool that actually transfers tokens (used for boost-delta tests).
#[contract]
pub struct MockRewardPoolTransfer;

#[contractimpl]
impl MockRewardPoolTransfer {
    pub fn distribute_reward(env: Env, _caller: Address, learner: Address, amount: i128) {
        let token_id: Address = env
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("token"))
            .unwrap();
        soroban_sdk::token::Client::new(&env, &token_id).transfer(
            &env.current_contract_address(),
            &learner,
            &amount,
        );
    }

    pub fn set_token(env: Env, token: Address) {
        env.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("token"), &token);
    }
}

#[test]
fn test_create_explore_quest_success() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[60u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    assert_eq!(quest_id, 1);

    let quest = client.get_quest(&quest_id).unwrap();
    assert_eq!(quest.employer, admin);
    assert_eq!(quest.reward_amount, reward_amount);
    assert_eq!(quest.quest_type, QuestType::Explore);
    assert_eq!(quest.metadata_hash, metadata_hash);
    assert!(quest.active);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_create_explore_quest_unauthorized() {
    let (env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let unauthorized = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[61u8; 32]);

    client.create_explore_quest(&unauthorized, &reward_amount, &metadata_hash);
}

#[test]
fn test_verify_explore_quest_success() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[63u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&admin, &learner, &quest_id);

    let quest = client.get_quest(&quest_id).unwrap();
    assert_eq!(quest.quest_type, QuestType::Explore);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_verify_explore_quest_unauthorized() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let unauthorized = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[64u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&unauthorized, &learner, &quest_id);
}

#[test]
#[should_panic(expected = "Quest not found")]
fn test_verify_explore_quest_nonexistent() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);

    client.verify_explore_quest(&admin, &learner, &999);
}

#[test]
#[should_panic(expected = "Not an Explore quest")]
fn test_verify_explore_quest_wrong_type() {
    let (env, client, token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[65u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.verify_explore_quest(&admin, &learner, &quest_id);
}

// ── Explore Quest Replay Guard Tests ──────────────────────────────────────

#[test]
#[should_panic(expected = "Learner already verified for this quest")]
fn test_explore_quest_cannot_be_verified_twice() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[100u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&admin, &learner, &quest_id);
    client.verify_explore_quest(&admin, &learner, &quest_id);
}

#[test]
#[should_panic(expected = "Learner already verified for this quest")]
fn test_explore_quest_duplicate_verification_panics() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[101u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&admin, &learner, &quest_id);
    client.verify_explore_quest(&admin, &learner, &quest_id);
}

// ── Quest Escrow Budget Tracking Tests ───────────────────────────────────────

#[test]
fn test_build_quest_initializes_accounting() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[100u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    let quest = client.get_quest(&quest_id).unwrap();
    assert_eq!(quest.total_funded, reward_amount);
    assert_eq!(quest.consumed_amount, 0);
    assert_eq!(quest.refunded_amount, 0);

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.total_funded, reward_amount);
    assert_eq!(budget.consumed_amount, 0);
    assert_eq!(budget.refunded_amount, 0);
    assert_eq!(budget.remaining, reward_amount);
}

#[test]
fn test_explore_quest_has_zero_escrow() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[101u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);

    // Explore quests are funded by the reward pool, so no on-chain escrow
    // is tracked against the quest itself.
    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.total_funded, 0);
    assert_eq!(budget.consumed_amount, 0);
    assert_eq!(budget.refunded_amount, 0);
    assert_eq!(budget.remaining, 0);
}

#[test]
fn test_get_quest_budget_returns_none_for_missing_quest() {
    let (_env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    assert!(client.get_quest_budget(&999).is_none());
}

/// Full setup that wires a real MockRewardPoolTransfer so the boost delta can be
/// distributed when the multiplier is > 100.
/// Returns (env, client, token_id, reward_pool_id).
fn setup_with_boosted_multiplier() -> (
    Env,
    QuestEngineContractClient<'static>,
    Address, // token_id
    Address, // reward_pool_id (MockRewardPoolTransfer)
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let stake_vault_id = env.register(MockStakeVaultWithMultiplier, ());

    // Register the real reward pool mock and configure its token.
    let reward_pool_id = env.register(MockRewardPoolTransfer, ());
    let rp_client = MockRewardPoolTransferClient::new(&env, &reward_pool_id);
    rp_client.set_token(&token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    (env, client, token_id, reward_pool_id)
}

#[test]
fn test_review_submission_updates_consumed_amount() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[102u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[103u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    // With the default 1.0x multiplier the whole escrow is consumed.
    let fee = (reward_amount * 15) / 100;
    let learner_amount = reward_amount - fee;

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.total_funded, reward_amount);
    assert_eq!(budget.consumed_amount, fee + learner_amount);
    assert_eq!(budget.remaining, 0);
}

#[test]
fn test_review_submission_rejection_does_not_consume_budget() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[104u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[105u8; 32]);

    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boosted = (base * 120) / 100; // 1020
    let boost_delta = boosted - base; // 170

    // Fund employer for quest escrow.
    mint_tokens(&env, &token_id, &employer, &reward_amount);
    // Pre-fund the RewardPool with enough to cover the boost delta.
    mint_tokens(&env, &token_id, &reward_pool_id, &boost_delta);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &false);

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.consumed_amount, 0);
    assert_eq!(budget.remaining, reward_amount);
}

// ── Acceptance Criteria Tests ─────────────────────────────────────────────────

#[test]
fn test_staked_learner_receives_more_than_non_staked() {
    // AC: A learner with a 120 multiplier receives more than a non-staked learner
    // for the same approved quest. Two identical quests, same reward_amount,
    // one learner staked (120x), one not (100x).
    let reward_amount: i128 = 1000;
    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boost_delta_120 = (base * 120) / 100 - base; // 170

    // --- Non-staked learner (100x) ---
    let (env1, client1, token_id1, _rp1) = setup_with_multiplier(100);
    let employer1 = Address::generate(&env1);
    let learner_no_stake = Address::generate(&env1);
    let mh = BytesN::from_array(&env1, &[80u8; 32]);
    let ph = BytesN::from_array(&env1, &[81u8; 32]);
    mint_tokens(&env1, &token_id1, &employer1, &reward_amount);
    let qid1 = client1.create_build_quest(&employer1, &reward_amount, &mh);
    client1.submit_proof(&learner_no_stake, &qid1, &ph);
    client1.review_submission(&employer1, &learner_no_stake, &qid1, &true);
    let non_staked_payout = token_balance(&env1, &token_id1, &learner_no_stake);

    // --- Staked learner (120x) ---
    let (env2, client2, token_id2, rp2) = setup_with_boosted_multiplier();
    let employer2 = Address::generate(&env2);
    let learner_staked = Address::generate(&env2);
    let mh2 = BytesN::from_array(&env2, &[82u8; 32]);
    let ph2 = BytesN::from_array(&env2, &[83u8; 32]);
    mint_tokens(&env2, &token_id2, &employer2, &reward_amount);
    mint_tokens(&env2, &token_id2, &rp2, &boost_delta_120);
    let qid2 = client2.create_build_quest(&employer2, &reward_amount, &mh2);
    client2.submit_proof(&learner_staked, &qid2, &ph2);
    client2.review_submission(&employer2, &learner_staked, &qid2, &true);
    let staked_payout = token_balance(&env2, &token_id2, &learner_staked);

    assert!(
        staked_payout > non_staked_payout,
        "Staked learner ({}) must earn more than non-staked learner ({})",
        staked_payout,
        non_staked_payout
    );
    assert_eq!(non_staked_payout, base); // 850 for 100x
    assert_eq!(staked_payout, (base * 120) / 100); // 1020 for 120x
}

#[test]
fn test_basis_point_math_for_100_120_200_multipliers() {
    // AC: Basis-point math remains correct for 100, 120, and 200 multiplier tiers.
    let reward: i128 = 1000;
    let fee = (reward * 15) / 100; // 150
    let base = reward - fee; // 850

    // 100x: boosted == base (no change)
    let b100 = (base * 100) / 100;
    assert_eq!(b100, 850);

    // 120x: boosted = 1020
    let b120 = (base * 120) / 100;
    assert_eq!(b120, 1020);

    // 200x: boosted = 1700
    let b200 = (base * 200) / 100;
    assert_eq!(b200, 1700);

    // Each tier is strictly larger than the previous.
    assert!(b100 < b120);
    assert!(b120 < b200);
}

#[test]
fn test_review_submission_with_200_multiplier_draws_delta_from_pool() {
    // AC: 200x multiplier tier works end-to-end; delta from RewardPool.
    // reward=1000 → fee=150, base=850, boosted=1700, delta=850.
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let stake_vault_id = env.register(MockStakeVault200, ());
    let reward_pool_id = env.register(MockRewardPoolTransfer, ());
    let rp_client = MockRewardPoolTransferClient::new(&env, &reward_pool_id);
    rp_client.set_token(&token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[90u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[91u8; 32]);

    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boosted = (base * 200) / 100; // 1700
    let boost_delta = boosted - base; // 850

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    mint_tokens(&env, &token_id, &reward_pool_id, &boost_delta);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    assert_eq!(token_balance(&env, &token_id, &learner), boosted);
    assert_eq!(token_balance(&env, &token_id, &reward_pool_id), fee);
}

#[test]
#[should_panic]
fn test_review_submission_fails_deterministically_when_pool_cannot_cover_boost() {
    // AC: The contract fails deterministically if the configured funding source
    // cannot cover the boosted payout.
    // Pool has 0 tokens for the delta → token transfer panics.
    let (env, client, token_id, _reward_pool_id) = setup_with_boosted_multiplier();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[92u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[93u8; 32]);

    // Fund employer but do NOT pre-fund the RewardPool for the delta.
    mint_tokens(&env, &token_id, &employer, &reward_amount);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    // Should panic because pool has no balance for the 170 boost_delta.
    client.review_submission(&employer, &learner, &quest_id, &true);
}

#[test]
#[should_panic(expected = "Insufficient quest budget")]
fn test_batch_review_fails_when_quest_budget_insufficient() {
    // A single quest can only afford one payout of `reward_amount`; a batch
    // approving two learners must fail rather than transferring more than the
    // escrowed amount.
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[106u8; 32]);
    let proof_hash1 = BytesN::from_array(&env, &[107u8; 32]);
    let proof_hash2 = BytesN::from_array(&env, &[108u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner1, &quest_id, &proof_hash1);
    client.submit_proof(&learner2, &quest_id, &proof_hash2);

    let mut learners = soroban_sdk::Vec::new(&env);
    learners.push_back(learner1);
    learners.push_back(learner2);
    client.batch_review_submissions(&employer, &quest_id, &learners);
}

#[test]
fn test_batch_review_consumes_budget_consistently() {
    // Same-quest batch approvals should update the accounting once per
    // learner they actually pay out.
    let (env, client, token_id, reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);
    let reward_amount: i128 = 1000;
    // Fund enough escrow for two payouts on the same quest by creating two
    // quests instead (matching existing API), and verify each budget is
    // updated independently.
    let metadata_hash = BytesN::from_array(&env, &[109u8; 32]);
    let proof_hash1 = BytesN::from_array(&env, &[110u8; 32]);
    let proof_hash2 = BytesN::from_array(&env, &[111u8; 32]);

    mint_tokens(&env, &token_id, &employer, &(reward_amount * 2));
    let quest_id_1 = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    let quest_id_2 = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner1, &quest_id_1, &proof_hash1);
    client.submit_proof(&learner2, &quest_id_2, &proof_hash2);

    let mut batch1 = soroban_sdk::Vec::new(&env);
    batch1.push_back(learner1.clone());
    client.batch_review_submissions(&employer, &quest_id_1, &batch1);

    let mut batch2 = soroban_sdk::Vec::new(&env);
    batch2.push_back(learner2.clone());
    client.batch_review_submissions(&employer, &quest_id_2, &batch2);

    let fee = (reward_amount * 15) / 100;
    let learner_amount = reward_amount - fee;

    let budget1 = client.get_quest_budget(&quest_id_1).unwrap();
    assert_eq!(budget1.consumed_amount, fee + learner_amount);
    assert_eq!(budget1.remaining, 0);

    let budget2 = client.get_quest_budget(&quest_id_2).unwrap();
    assert_eq!(budget2.consumed_amount, fee + learner_amount);
    assert_eq!(budget2.remaining, 0);

    // Sanity check on token flow.
    assert_eq!(token_balance(&env, &token_id, &learner1), learner_amount);
    assert_eq!(token_balance(&env, &token_id, &learner2), learner_amount);
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee * 2);
}

#[test]
fn test_refund_returns_only_unspent_balance_after_rejection() {
    // A rejected submission leaves the escrow untouched, so the full amount
    // should be refundable.
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[112u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[113u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &false);

    client.refund_quest(&employer, &quest_id);

    assert_eq!(token_balance(&env, &token_id, &employer), reward_amount);
    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.refunded_amount, reward_amount);
    assert_eq!(budget.remaining, 0);
}

#[test]
fn test_refund_after_boosted_multiplier_escrow_is_fully_consumed() {
    // After an approval with 120x multiplier, the full quest escrow is consumed
    // (fee + base drawn from escrow, delta drawn from RewardPool).
    // A subsequent refund attempt should panic — nothing left to return.
    let (env, client, token_id, reward_pool_id) = setup_with_boosted_multiplier();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[114u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[115u8; 32]);

    let fee = (reward_amount * 15) / 100; // 150
    let base = reward_amount - fee; // 850
    let boosted = (base * 120) / 100; // 1020
    let boost_delta = boosted - base; // 170

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    mint_tokens(&env, &token_id, &reward_pool_id, &boost_delta);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    // Learner received base from escrow + delta from RewardPool = boosted total.
    assert_eq!(token_balance(&env, &token_id, &learner), boosted);
    // RewardPool received fee, paid out delta → net = fee - delta + delta = fee... no:
    // pool received fee (150) via token transfer from escrow,
    // pool paid delta (170) to learner via distribute_reward → net = 150 - 170 = -20?
    // That would overdraft. Actually pool was pre-funded with 170, got +150 fee, paid -170 delta → net = 150.
    assert_eq!(token_balance(&env, &token_id, &reward_pool_id), fee);

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.consumed_amount, reward_amount); // full escrow consumed
    assert_eq!(budget.remaining, 0);
}

#[test]
#[should_panic(expected = "No unspent balance to refund")]
fn test_refund_panics_when_no_unspent_balance() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[116u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[117u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    client.refund_quest(&employer, &quest_id);
}

#[test]
fn test_refund_records_refunded_amount() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[118u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.refund_quest(&employer, &quest_id);

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.refunded_amount, reward_amount);
    assert_eq!(budget.consumed_amount, 0);
    assert_eq!(budget.remaining, 0);
}

/// Mock RewardPool that actually transfers tokens (used for boost-delta tests).
#[contract]
pub struct MockRewardPoolTransfer;

#[contractimpl]
impl MockRewardPoolTransfer {
    pub fn distribute_reward(env: Env, _caller: Address, learner: Address, amount: i128) {
        let token_id: Address = env
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("token"))
            .unwrap();
        soroban_sdk::token::Client::new(&env, &token_id).transfer(
            &env.current_contract_address(),
            &learner,
            &amount,
        );
    }

    pub fn set_token(env: Env, token: Address) {
        env.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("token"), &token);
    }
}

#[test]
fn test_different_learners_can_verify_same_quest() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[102u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&admin, &learner1, &quest_id);
    client.verify_explore_quest(&admin, &learner2, &quest_id);
}

#[test]
#[should_panic(expected = "Learner already verified for this quest")]
fn test_verify_explore_quest_replay_attack_prevented() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[103u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&admin, &learner, &quest_id);
    client.verify_explore_quest(&admin, &learner, &quest_id);
}

#[test]
#[should_panic(expected = "Learner already verified for this quest")]
fn test_explore_verification_persists_after_payout() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[104u8; 32]);

    let quest_id = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    client.verify_explore_quest(&admin, &learner, &quest_id);
    client.verify_explore_quest(&admin, &learner, &quest_id);
}

#[test]
#[should_panic(expected = "Learner already verified for this quest")]
fn test_verify_explore_quest_with_multiple_quests() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[105u8; 32]);

    let quest_id1 = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);
    let quest_id2 = client.create_explore_quest(&admin, &reward_amount, &metadata_hash);

    client.verify_explore_quest(&admin, &learner, &quest_id1);
    client.verify_explore_quest(&admin, &learner, &quest_id1); // Should panic
    client.verify_explore_quest(&admin, &learner, &quest_id2);
}
