use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Events},
    Address, BytesN, Env,
};

use crate::types::{QuestType, SubmissionStatus};
use crate::{QuestEngineContract, QuestEngineContractClient};

// ── Mock StakeVault Contract ─────────────────────────────────────────────────

#[contract]
pub struct MockStakeVault;

#[contractimpl]
impl MockStakeVault {
    pub fn get_multiplier(_env: Env, _learner: Address) -> u32 {
        100
    }
}

/// Mock StakeVault that returns a custom multiplier
#[contract]
pub struct MockStakeVaultWithMultiplier;

#[contractimpl]
impl MockStakeVaultWithMultiplier {
    pub fn get_multiplier(_env: Env, _learner: Address) -> u32 {
        120
    }
}

/// Mock StakeVault returning a 200x (2.0x) multiplier tier.
#[contract]
pub struct MockStakeVault200;

#[contractimpl]
impl MockStakeVault200 {
    pub fn get_multiplier(_env: Env, _learner: Address) -> u32 {
        200
    }
}

#[contract]
pub struct MockStakeVault80;

#[contractimpl]
impl MockStakeVault80 {
    pub fn get_multiplier(_env: Env, _learner: Address) -> u32 {
        80 // 0.8x multiplier (penalty)
    }
}

// ── Mock RewardPool Contract ─────────────────────────────────────────────────

#[contract]
pub struct MockRewardPool;

#[contractimpl]
impl MockRewardPool {
    pub fn distribute_reward(_env: Env, _caller: Address, _learner: Address, _amount: i128) {
        // No-op for testing
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

fn setup_with_multiplier(
    multiplier: u32,
) -> (Env, QuestEngineContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    // Register custom stake vault based on multiplier
    let stake_vault_id = if multiplier == 120 {
        env.register(MockStakeVaultWithMultiplier, ())
    } else if multiplier == 200 {
        env.register(MockStakeVault200, ())
    } else if multiplier == 80 {
        env.register(MockStakeVault80, ())
    } else {
        env.register(MockStakeVault, ())
    };

    let reward_pool_id = env.register(MockRewardPool, ());

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    (env, client, token_id, reward_pool_id)
}

fn setup_with_boosted_multiplier() -> (Env, QuestEngineContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let stake_vault_id = env.register(MockStakeVaultWithMultiplier, ());

    let reward_pool_id = env.register(MockRewardPoolTransfer, ());
    let rp_client = MockRewardPoolTransferClient::new(&env, &reward_pool_id);
    rp_client.set_token(&token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id, &reward_pool_id, &stake_vault_id);

    (env, client, token_id, reward_pool_id)
}

fn mint_tokens(env: &Env, token_id: &Address, to: &Address, amount: &i128) {
    let sac_client = soroban_sdk::token::StellarAssetClient::new(env, token_id);
    sac_client.mint(to, amount);
}

fn token_balance(env: &Env, token_id: &Address, of: &Address) -> i128 {
    soroban_sdk::token::Client::new(env, token_id).balance(of)
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

#[test]
fn test_create_build_quest_emits_event() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);

    client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
fn test_create_build_quest_increments_ids() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[3u8; 32]);

    mint_tokens(&env, &token_id, &employer, &3000);

    let id1 = client.create_build_quest(&employer, &1000, &metadata_hash);
    let id2 = client.create_build_quest(&employer, &1000, &metadata_hash);
    let id3 = client.create_build_quest(&employer, &1000, &metadata_hash);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);

    for id in [id1, id2, id3] {
        let quest = client.get_quest(&id).unwrap();
        assert_eq!(quest.quest_type, QuestType::Build);
        assert!(quest.active);
    }

    assert_eq!(token_balance(&env, &token_id, &client.address), 3000);
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_create_quest_without_init_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let employer = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.create_build_quest(&employer, &100, &metadata_hash);
}

#[test]
fn test_get_quest_returns_none_for_nonexistent() {
    let (_env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    assert_eq!(client.get_quest(&999), None);
}

#[test]
fn test_create_build_quest_multiple_employers() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer1 = Address::generate(&env);
    let employer2 = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[4u8; 32]);

    mint_tokens(&env, &token_id, &employer1, &500);
    mint_tokens(&env, &token_id, &employer2, &700);

    let id1 = client.create_build_quest(&employer1, &500, &metadata_hash);
    let id2 = client.create_build_quest(&employer2, &700, &metadata_hash);

    let quest1 = client.get_quest(&id1).unwrap();
    let quest2 = client.get_quest(&id2).unwrap();

    assert_eq!(quest1.employer, employer1);
    assert_eq!(quest1.reward_amount, 500);
    assert_eq!(quest2.employer, employer2);
    assert_eq!(quest2.reward_amount, 700);

    assert_eq!(token_balance(&env, &token_id, &client.address), 1200);
}

// ── submit_proof Tests ───────────────────────────────────────────────────────

#[test]
fn test_submit_proof_success() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[5u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[6u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);

    let submission = client.get_submission(&learner, &quest_id).unwrap();
    assert_eq!(submission.proof_hash, proof_hash);
    assert_eq!(submission.status, SubmissionStatus::Pending);
}

#[test]
fn test_submit_proof_emits_event() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[7u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[8u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
#[should_panic(expected = "Quest not found")]
fn test_submit_proof_nonexistent_quest_panics() {
    let (_env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let learner = Address::generate(&_env);
    let proof_hash = BytesN::from_array(&_env, &[9u8; 32]);

    client.submit_proof(&learner, &999, &proof_hash);
}

#[test]
#[should_panic(expected = "Submission already exists")]
fn test_submit_proof_duplicate_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[14u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[15u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
}

#[test]
fn test_get_submission_returns_none_for_nonexistent() {
    let (_env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let learner = Address::generate(&_env);
    assert_eq!(client.get_submission(&learner, &999), None);
}

// ── review_submission Tests ──────────────────────────────────────────────────

#[test]
fn test_review_submission_approve_success() {
    let (env, client, token_id, reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[16u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[17u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);

    assert_eq!(
        token_balance(&env, &token_id, &client.address),
        reward_amount
    );
    assert_eq!(token_balance(&env, &token_id, &learner), 0);

    client.review_submission(&employer, &learner, &quest_id, &true);

    let fee = (reward_amount * 15) / 100;
    let learner_amount = reward_amount - fee;

    assert_eq!(token_balance(&env, &token_id, &client.address), 0);
    assert_eq!(token_balance(&env, &token_id, &learner), learner_amount);
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee);
}

#[test]
fn test_review_submission_reject_success() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[18u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[19u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);

    assert_eq!(
        token_balance(&env, &token_id, &client.address),
        reward_amount
    );
    assert_eq!(token_balance(&env, &token_id, &learner), 0);

    client.review_submission(&employer, &learner, &quest_id, &false);

    assert_eq!(
        token_balance(&env, &token_id, &client.address),
        reward_amount
    );
    assert_eq!(token_balance(&env, &token_id, &learner), 0);

    let submission = client.get_submission(&learner, &quest_id).unwrap();
    assert_eq!(submission.status, SubmissionStatus::Rejected);
}

#[test]
fn test_review_submission_emits_event() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[20u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[21u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
#[should_panic(expected = "Quest not found")]
fn test_review_submission_nonexistent_quest_panics() {
    let (_env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&_env);
    let learner = Address::generate(&_env);

    client.review_submission(&employer, &learner, &999, &true);
}

#[test]
#[should_panic(expected = "Only the quest employer can review submissions")]
fn test_review_submission_wrong_employer_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let wrong_employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[22u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[23u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);

    client.review_submission(&wrong_employer, &learner, &quest_id, &true);
}

#[test]
#[should_panic(expected = "Submission not found")]
fn test_review_submission_nonexistent_submission_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[24u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.review_submission(&employer, &learner, &quest_id, &true);
}

#[test]
#[should_panic(expected = "Submission is not pending review")]
fn test_review_submission_already_reviewed_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[25u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[26u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);
    client.review_submission(&employer, &learner, &quest_id, &false);
}

// ── Refund Tests ─────────────────────────────────────────────────────────────

#[test]
fn test_refund_quest_success() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[30u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    assert_eq!(
        token_balance(&env, &token_id, &client.address),
        reward_amount
    );
    assert_eq!(token_balance(&env, &token_id, &employer), 0);

    client.refund_quest(&employer, &quest_id);

    assert_eq!(token_balance(&env, &token_id, &client.address), 0);
    assert_eq!(token_balance(&env, &token_id, &employer), reward_amount);

    let quest = client.get_quest(&quest_id).unwrap();
    assert!(!quest.active);
}

#[test]
#[should_panic(expected = "No unspent balance to refund")]
fn test_refund_quest_after_full_payout_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[33u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[34u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    client.refund_quest(&employer, &quest_id);
}

#[test]
fn test_refund_quest_after_rejected_submission_still_succeeds() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[35u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[36u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &false);

    client.refund_quest(&employer, &quest_id);

    assert_eq!(token_balance(&env, &token_id, &client.address), 0);
    assert_eq!(token_balance(&env, &token_id, &employer), reward_amount);

    let quest = client.get_quest(&quest_id).unwrap();
    assert!(!quest.active);
}

#[test]
fn test_refund_quest_before_submission_approval_still_succeeds() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[37u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[38u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.refund_quest(&employer, &quest_id);

    assert_eq!(token_balance(&env, &token_id, &client.address), 0);
    assert_eq!(token_balance(&env, &token_id, &employer), reward_amount);

    let submission = client.get_submission(&learner, &quest_id).unwrap();
    assert_eq!(submission.status, SubmissionStatus::Pending);

    let quest = client.get_quest(&quest_id).unwrap();
    assert!(!quest.active);
}

#[test]
#[should_panic(expected = "Quest already inactive")]
fn test_refund_quest_already_inactive_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[31u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.refund_quest(&employer, &quest_id);
    client.refund_quest(&employer, &quest_id);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_refund_quest_wrong_employer_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let wrong_employer = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[32u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.refund_quest(&wrong_employer, &quest_id);
}

// ── Staking Multiplier Tests ────────────────────────────────────────────────

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

    let fee = (reward_amount * 15) / 100;
    let base_amount = reward_amount - fee;

    assert_eq!(token_balance(&env, &token_id, &learner), base_amount);
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee);
}

#[test]
fn test_review_submission_with_120_multiplier() {
    let (env, client, token_id, reward_pool_id) = setup_with_boosted_multiplier();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[52u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[53u8; 32]);

    let fee = (reward_amount * 15) / 100;
    let base = reward_amount - fee;
    let boosted = (base * 120) / 100;
    let boost_delta = boosted - base;

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    mint_tokens(&env, &token_id, &reward_pool_id, &boost_delta);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    client.review_submission(&employer, &learner, &quest_id, &true);

    assert_eq!(token_balance(&env, &token_id, &learner), boosted);
    assert_eq!(token_balance(&env, &token_id, &reward_pool_id), fee);
    assert!(boosted > base);
}

#[test]
fn test_multiplier_math_correctness() {
    let test_cases = [
        (1000i128, 150i128, 850i128, 1020i128),
        (5000i128, 750i128, 4250i128, 5100i128),
        (10000i128, 1500i128, 8500i128, 10200i128),
    ];

    for (reward, expected_fee, expected_base, expected_boosted) in test_cases {
        let fee = (reward * 15) / 100;
        let base = reward - fee;
        let boosted = (base * 120) / 100;

        assert_eq!(fee, expected_fee);
        assert_eq!(base, expected_base);
        assert_eq!(boosted, expected_boosted);
    }
}

// ── Acceptance Criteria Tests ──────────────────────────────────────────────

#[test]
fn test_staked_learner_receives_more_than_non_staked() {
    let reward_amount: i128 = 1000;
    let fee = (reward_amount * 15) / 100;
    let base = reward_amount - fee;
    let boost_delta_120 = (base * 120) / 100 - base;

    // Non-staked learner (100x)
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

    // Staked learner (120x)
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

    assert!(staked_payout > non_staked_payout);
    assert_eq!(non_staked_payout, base);
    assert_eq!(staked_payout, (base * 120) / 100);
}

#[test]
fn test_basis_point_math_for_100_120_200_multipliers() {
    let reward: i128 = 1000;
    let fee = (reward * 15) / 100;
    let base = reward - fee;

    let b100 = (base * 100) / 100;
    assert_eq!(b100, 850);

    let b120 = (base * 120) / 100;
    assert_eq!(b120, 1020);

    let b200 = (base * 200) / 100;
    assert_eq!(b200, 1700);

    assert!(b100 < b120);
    assert!(b120 < b200);
}

#[test]
fn test_review_submission_with_200_multiplier_draws_delta_from_pool() {
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

    let fee = (reward_amount * 15) / 100;
    let base = reward_amount - fee;
    let boosted = (base * 200) / 100;
    let boost_delta = boosted - base;

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
    let (env, client, token_id, _reward_pool_id) = setup_with_boosted_multiplier();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[92u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[93u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);

    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    client.review_submission(&employer, &learner, &quest_id, &true);
}

#[test]
fn test_review_submission_with_80_multiplier() {
    // Test with 80 multiplier (0.8x penalty)
    let (env, client, token_id, reward_pool) = setup_with_multiplier(80);
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[54u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[55u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);

    client.review_submission(&employer, &learner, &quest_id, &true);

    let fee = (reward_amount * 15) / 100;
    let base = reward_amount - fee;
    let expected = (base * 80) / 100; // 80% of base (penalty)

    assert_eq!(token_balance(&env, &token_id, &learner), expected);
    assert_eq!(
        token_balance(&env, &token_id, &reward_pool),
        fee + (base - expected)
    );
}

// ── batch_review_submissions Tests ──────────────────────────────────────────

#[test]
fn test_batch_review_submissions_pays_all_learners() {
    let (env, client, token_id, reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);
    let reward_amount: i128 = 1_000;
    let metadata_hash = BytesN::from_array(&env, &[1u8; 32]);

    mint_tokens(&env, &token_id, &employer, &(reward_amount * 2));
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    let quest_id2 = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    client.submit_proof(&learner1, &quest_id, &metadata_hash);
    client.submit_proof(&learner2, &quest_id2, &metadata_hash);

    let mut learners1 = soroban_sdk::Vec::new(&env);
    learners1.push_back(learner1.clone());
    client.batch_review_submissions(&employer, &quest_id, &learners1);

    let mut learners2 = soroban_sdk::Vec::new(&env);
    learners2.push_back(learner2.clone());
    client.batch_review_submissions(&employer, &quest_id2, &learners2);

    let fee = (reward_amount * 15) / 100;
    let learner_amount = reward_amount - fee;

    assert_eq!(token_balance(&env, &token_id, &learner1), learner_amount);
    assert_eq!(token_balance(&env, &token_id, &learner2), learner_amount);
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee * 2);
}

#[test]
fn test_batch_review_submissions_single_learner() {
    let (env, client, token_id, reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[5u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &metadata_hash);

    let mut learners = soroban_sdk::Vec::new(&env);
    learners.push_back(learner.clone());
    client.batch_review_submissions(&employer, &quest_id, &learners);

    let fee = (reward_amount * 15) / 100;
    let learner_amount = reward_amount - fee;
    assert_eq!(token_balance(&env, &token_id, &learner), learner_amount);
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee);
}

#[test]
fn test_batch_review_submissions_emits_batch_reviewed_event() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 300;
    let metadata_hash = BytesN::from_array(&env, &[9u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &metadata_hash);

    let mut learners = soroban_sdk::Vec::new(&env);
    learners.push_back(learner.clone());
    client.batch_review_submissions(&employer, &quest_id, &learners);

    assert!(!env.events().all().is_empty());
}

#[test]
#[should_panic(expected = "Only the quest employer can review submissions")]
fn test_batch_review_wrong_employer_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let wrong_employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 200;
    let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &metadata_hash);

    let mut learners = soroban_sdk::Vec::new(&env);
    learners.push_back(learner.clone());
    client.batch_review_submissions(&wrong_employer, &quest_id, &learners);
}

#[test]
#[should_panic(expected = "Submission not found")]
fn test_batch_review_missing_submission_panics() {
    let (env, client, token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 200;
    let metadata_hash = BytesN::from_array(&env, &[3u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    let mut learners = soroban_sdk::Vec::new(&env);
    learners.push_back(learner.clone());
    client.batch_review_submissions(&employer, &quest_id, &learners);
}

// ── upgrade_contract Tests ──────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_upgrade_contract_non_admin_panics() {
    let (env, client, _token_id, _reward_pool, _admin, _stake_vault_id) = setup();
    let attacker = Address::generate(&env);
    let new_wasm_hash = BytesN::from_array(&env, &[0xabu8; 32]);
    client.upgrade_contract(&attacker, &new_wasm_hash);
}

// ── Explore Quest Tests ──────────────────────────────────────────────────────

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
fn test_create_explore_quest_increments_ids() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let metadata_hash = BytesN::from_array(&env, &[62u8; 32]);

    let id1 = client.create_explore_quest(&admin, &100, &metadata_hash);
    let id2 = client.create_explore_quest(&admin, &200, &metadata_hash);
    let id3 = client.create_explore_quest(&admin, &300, &metadata_hash);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);

    for id in [id1, id2, id3] {
        let quest = client.get_quest(&id).unwrap();
        assert_eq!(quest.quest_type, QuestType::Explore);
    }
}

#[test]
fn test_verify_explore_quest_success() {
    let (env, _client, token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let learner = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[63u8; 32]);

    let mock_reward_pool_id = env.register(MockRewardPool, ());
    let mock_stake_vault_id = env.register(MockStakeVault, ());

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);
    client.initialize(
        &admin,
        &token_id,
        &mock_reward_pool_id,
        &mock_stake_vault_id,
    );

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

#[test]
fn test_explore_quest_emits_event() {
    let (env, client, _token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[66u8; 32]);

    client.create_explore_quest(&admin, &reward_amount, &metadata_hash);

    let events = env.events().all();
    assert!(!events.is_empty());
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
    client.verify_explore_quest(&admin, &learner, &quest_id1);
    client.verify_explore_quest(&admin, &learner, &quest_id2);
}

// ── Mixed Quest Types Tests ─────────────────────────────────────────────────

#[test]
fn test_mixed_quest_types() {
    let (env, client, token_id, _reward_pool, admin, _stake_vault_id) = setup();
    let employer = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[67u8; 32]);

    mint_tokens(&env, &token_id, &employer, &1000);
    let build_id = client.create_build_quest(&employer, &1000, &metadata_hash);

    let explore_id = client.create_explore_quest(&admin, &500, &metadata_hash);

    let build_quest = client.get_quest(&build_id).unwrap();
    let explore_quest = client.get_quest(&explore_id).unwrap();

    assert_eq!(build_quest.quest_type, QuestType::Build);
    assert_eq!(explore_quest.quest_type, QuestType::Explore);
    assert_eq!(build_quest.employer, employer);
    assert_eq!(explore_quest.employer, admin);
}
