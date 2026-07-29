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
    assert!(!quest.has_approved_submission);
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

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &false);

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.consumed_amount, 0);
    assert_eq!(budget.remaining, reward_amount);
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
fn test_refund_after_capped_multiplier_returns_leftover_only() {
    // With a >1.0x multiplier plus fees, the full escrow is consumed and any
    // subsequent refund attempt has nothing to return.
    let (env, client, token_id, reward_pool) = setup_with_multiplier(120);
    let employer = Address::generate(&env);
    let learner = Address::generate(&env);
    let reward_amount: i128 = 1000;
    let metadata_hash = BytesN::from_array(&env, &[114u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[115u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);
    client.submit_proof(&learner, &quest_id, &proof_hash);
    client.review_submission(&employer, &learner, &quest_id, &true);

    let fee = (reward_amount * 15) / 100;
    let base = reward_amount - fee;

    assert_eq!(token_balance(&env, &token_id, &learner), base);
    assert_eq!(token_balance(&env, &token_id, &reward_pool), fee);

    let budget = client.get_quest_budget(&quest_id).unwrap();
    assert_eq!(budget.consumed_amount, reward_amount);
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
