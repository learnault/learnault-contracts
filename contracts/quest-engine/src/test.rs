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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    QuestEngineContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
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

fn mint_tokens(env: &Env, token_id: &Address, to: &Address, amount: &i128) {
    let sac_client = soroban_sdk::token::StellarAssetClient::new(env, token_id);
    sac_client.mint(to, amount);
}

fn token_balance(env: &Env, token_id: &Address, of: &Address) -> i128 {
    soroban_sdk::token::Client::new(env, token_id).balance(of)
}

fn setup_with_multiplier(multiplier: u32) -> (
    Env,
    QuestEngineContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

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
}

// ── Explore Quest Tests ─────────────────────────────────────────────────────

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
