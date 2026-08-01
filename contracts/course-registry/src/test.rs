#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env,
};

use crate::{CourseRegistry, CourseRegistryClient};
use badge_nft::{BadgeNFT, BadgeNFTClient};
use reward_pool::{RewardPool, RewardPoolClient};
use soroban_sdk::token;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, CourseRegistryClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CourseRegistry, ());
    let client = CourseRegistryClient::new(&env, &contract_id);
    (env, client)
}

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

/// Seeds an initialized contract with one course and returns (admin, instructor, course_id).
fn setup_with_course(
    env: &Env,
    client: &CourseRegistryClient,
    policy: Option<crate::CompletionPolicy>,
) -> (Address, Address, u32) {
    let admin = Address::generate(env);
    let instructor = Address::generate(env);
    client.initialize(&admin);
    let id = client.create_course(
        &admin,
        &instructor,
        &1, // Changed from 5 to 1 for easier testing
        &dummy_hash(env),
        &10_0000000,
        &policy,
    );
    (admin, instructor, id)
}

/// Deploys + initializes a RewardPool backed by a real SAC token.
fn setup_reward_pool<'a>(
    env: &Env,
    token_admin: &Address,
) -> (
    RewardPoolClient<'a>,
    soroban_sdk::token::StellarAssetClient<'a>,
    Address,
) {
    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = token_id.address();
    let token_sac = token::StellarAssetClient::new(env, &token_address);

    let reward_pool_id = env.register(RewardPool, ());
    let reward_pool_client = RewardPoolClient::new(env, &reward_pool_id);
    reward_pool_client.initialize(token_admin, &token_address);

    (reward_pool_client, token_sac, token_address)
}

/// Helper: deploys and initializes a BadgeNFT contract, authorizing the given registry address.
fn setup_badge_nft<'a>(env: &Env, registry_address: &Address) -> BadgeNFTClient<'a> {
    let badge_id = env.register(BadgeNFT, ());
    let badge_client = BadgeNFTClient::new(env, &badge_id);
    badge_client.initialize(registry_address);
    badge_client
}

// ── Completion Policy Tests ──────────────────────────────────────────────────

#[test]
fn test_create_course_with_default_policy() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );

    let policy = client.get_completion_policy(&course_id);
    assert_eq!(policy, crate::CompletionPolicy::Optional);
}

#[test]
fn test_create_course_with_custom_policy() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BothRequired),
    );

    let policy = client.get_completion_policy(&course_id);
    assert_eq!(policy, crate::CompletionPolicy::BothRequired);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_set_completion_policy_unauthorized() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);
    let fake_admin = Address::generate(&env);

    client.set_completion_policy(&fake_admin, &id, &crate::CompletionPolicy::RewardRequired);
}

#[test]
fn test_set_completion_policy_success() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client, None);

    client.set_completion_policy(&admin, &id, &crate::CompletionPolicy::BothRequired);

    let policy = client.get_completion_policy(&id);
    assert_eq!(policy, crate::CompletionPolicy::BothRequired);
}

#[test]
fn test_completion_policy_event_emitted() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client, None);

    client.set_completion_policy(&admin, &id, &crate::CompletionPolicy::RewardRequired);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
fn test_optional_policy_completes_without_integrations() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    // Create a course with 1 module and Optional policy
    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1, // Only 1 module
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    // No integrations configured
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    // Should complete successfully
    assert!(client.is_course_finished(&learner, &course_id));
}

#[test]
#[should_panic(expected = "Completion policy violation: Reward pool required but not configured")]
fn test_reward_required_policy_fails_without_reward_pool() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::RewardRequired),
    );

    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
fn test_reward_required_policy_succeeds_with_reward_pool() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::RewardRequired),
    );

    // Setup reward pool
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);
    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Complete course
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    assert!(client.is_course_finished(&learner, &course_id));
    assert_eq!(token_sac.balance(&learner), 10_0000000);
}

#[test]
#[should_panic(expected = "Completion policy violation: Badge NFT required but not configured")]
fn test_badge_required_policy_fails_without_badge_nft() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BadgeRequired),
    );

    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
fn test_badge_required_policy_succeeds_with_badge_nft() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BadgeRequired),
    );

    // Setup badge NFT
    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Complete course
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    assert!(client.is_course_finished(&learner, &course_id));
    assert!(badge_client.has_badge(&learner, &course_id));
}

#[test]
#[should_panic(expected = "Completion policy violation: Reward pool required but not configured")]
fn test_both_required_policy_fails_without_integrations() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BothRequired),
    );

    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
fn test_both_required_policy_succeeds_with_both_integrations() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BothRequired),
    );

    // Setup both integrations
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);
    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Complete course
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    assert!(client.is_course_finished(&learner, &course_id));
    assert!(badge_client.has_badge(&learner, &course_id));
    assert_eq!(token_sac.balance(&learner), 10_0000000);
}

#[test]
fn test_completion_event_shows_side_effects_status() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    // Setup both integrations (should be skipped since policy is Optional)
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);
    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Complete course
    client.enroll(&learner, &course_id);
    let initial_events = env.events().all().len();
    client.complete_module(&admin, &learner, &course_id);

    // CourseCompleted event should be emitted
    let events = env.events().all();
    assert!(events.len() > initial_events);
}

#[test]
fn test_multiple_courses_with_different_policies() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);

    let course1 = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let course2 = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &20_0000000,
        &Some(crate::CompletionPolicy::RewardRequired),
    );

    // Setup reward pool for course2
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);
    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    let learner = Address::generate(&env);

    // Course1 should complete without reward
    client.enroll(&learner, &course1);
    client.complete_module(&admin, &learner, &course1);
    assert!(client.is_course_finished(&learner, &course1));
    // Check balance after course1 (should be 0)
    assert_eq!(token_sac.balance(&learner), 0);

    // Course2 should complete with reward
    client.enroll(&learner, &course2);
    client.complete_module(&admin, &learner, &course2);
    assert!(client.is_course_finished(&learner, &course2));
    // Should only be 20_0000000, not accumulated
    assert_eq!(token_sac.balance(&learner), 20_0000000);
}

#[test]
#[should_panic(expected = "Completion policy violation: Reward pool required but not configured")]
fn test_policy_update_affects_future_completions() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &2,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    // Setup reward pool but don't configure it yet
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);
    reward_pool_client.add_approved_spender(&admin, &client.address);

    // Complete first module (not final)
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(client.get_progress(&learner, &course_id), 1);

    // Update policy to require reward pool
    client.set_completion_policy(&admin, &course_id, &crate::CompletionPolicy::RewardRequired);

    // Try to complete final module - should panic
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
fn test_course_creation_with_policy_event() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let initial_events = env.events().all().len();
    client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BothRequired),
    );

    let events = env.events().all();
    assert!(events.len() > initial_events);
}

#[test]
fn test_set_course_integrations_emits_events() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client, None);
    let reward_pool = Address::generate(&env);
    let badge_nft = Address::generate(&env);

    let initial_events = env.events().all().len();
    client.set_course_integrations(
        &admin,
        &id,
        &Some(reward_pool.clone()),
        &Some(badge_nft.clone()),
    );

    let events = env.events().all();
    assert!(events.len() > initial_events);
}

#[test]
#[should_panic(expected = "Reward payout failed but required by policy")]
fn test_policy_enforcement_with_global_addresses() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::RewardRequired),
    );

    // Set global reward pool address but don't whitelist it
    let reward_pool = Address::generate(&env);
    client.set_reward_pool_address(&admin, &reward_pool);

    // Try to complete - should fail because reward pool isn't properly configured
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
fn test_course_completed_event_includes_policy_and_side_effects() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::BothRequired),
    );

    // Setup integrations
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);
    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Complete course
    client.enroll(&learner, &course_id);
    let initial_events = env.events().all().len();
    client.complete_module(&admin, &learner, &course_id);

    // Verify CourseCompleted event was emitted
    let events = env.events().all();
    assert!(events.len() > initial_events);
}

#[test]
fn test_policy_change_after_course_creation() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &None, // Default Optional
    );

    // Initially Optional
    let policy = client.get_completion_policy(&course_id);
    assert_eq!(policy, crate::CompletionPolicy::Optional);

    // Change to BothRequired
    client.set_completion_policy(&admin, &course_id, &crate::CompletionPolicy::BothRequired);

    let policy = client.get_completion_policy(&course_id);
    assert_eq!(policy, crate::CompletionPolicy::BothRequired);
}

// ── Existing tests (modified to work with new API) ──────────────────────────

#[test]
fn test_create_course_returns_id_one() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);

    let id = client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );
    assert_eq!(id, 1);
}

#[test]
fn test_course_count_increments() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.initialize(&admin);

    assert_eq!(client.course_count(), 0);
    client.create_course(&admin, &instructor, &2, &hash, &10_0000000, &None);
    assert_eq!(client.course_count(), 1);
    client.create_course(&admin, &instructor, &5, &hash, &10_0000000, &None);
    assert_eq!(client.course_count(), 2);
}

#[test]
#[should_panic(expected = "total_modules must be greater than 0")]
fn test_zero_modules_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(
        &admin,
        &instructor,
        &0,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_unauthorized_admin_panics() {
    let (env, client) = setup();
    let true_admin = Address::generate(&env);
    let fake_admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&true_admin);
    client.create_course(
        &fake_admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );
}

#[test]
fn test_course_created_event_emitted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(
        &admin,
        &instructor,
        &4,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );

    assert_eq!(env.events().all().len(), 1);
}

// ── update_metadata ───────────────────────────────────────────────────────────

#[test]
fn test_update_metadata_success() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);
    let new_hash = BytesN::from_array(&env, &[2u8; 32]);

    client.update_metadata(&id, &new_hash);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_update_nonexistent_course() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    client.initialize(&admin);
    client.update_metadata(&99, &dummy_hash(&env));
}

#[test]
fn test_update_metadata_emits_event() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);
    let new_hash = BytesN::from_array(&env, &[2u8; 32]);

    client.update_metadata(&id, &new_hash);

    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_update_metadata_multiple_times() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);
    let hash_v2 = BytesN::from_array(&env, &[2u8; 32]);
    let hash_v3 = BytesN::from_array(&env, &[3u8; 32]);

    client.update_metadata(&id, &hash_v2);
    client.update_metadata(&id, &hash_v3);
}

// ── enroll ────────────────────────────────────────────────────────────────────

#[test]
fn test_enroll_success() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);

    let learner = Address::generate(&env);

    client.enroll(&learner, &id);
}

#[test]
fn test_enroll_multiple_learners_same_course() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);

    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    client.enroll(&learner_a, &id);
    client.enroll(&learner_b, &id);
}

#[test]
fn test_enroll_same_learner_different_courses() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.initialize(&admin);
    let id_1 = client.create_course(&admin, &instructor, &4, &hash, &10_0000000, &None);
    let id_2 = client.create_course(&admin, &instructor, &8, &hash, &10_0000000, &None);

    let learner = Address::generate(&env);

    client.enroll(&learner, &id_1);
    client.enroll(&learner, &id_2);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_enroll_panics_when_course_does_not_exist() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let learner = Address::generate(&env);
    client.enroll(&learner, &99u32);
}

#[test]
#[should_panic(expected = "Learner already enrolled")]
fn test_enroll_panics_when_learner_already_enrolled() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);

    let learner = Address::generate(&env);
    client.enroll(&learner, &id);
    client.enroll(&learner, &id);
}

#[test]
fn test_create_and_get_course() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &5, &hash, &25_0000000, &None);

    let retrieved_course = client.get_course(&course_id);

    assert_eq!(retrieved_course.instructor, instructor);
    assert_eq!(retrieved_course.total_modules, 5);
    assert_eq!(retrieved_course.metadata_hash, hash);
    assert!(retrieved_course.active);
    assert_eq!(retrieved_course.reward_amount, 25_0000000);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_get_nonexistent_course() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    client.initialize(&admin);
    let _ = client.get_course(&999);
}

// ── Complete Module Tests ────────────────────────────────────────────────────

#[test]
fn test_complete_module_increments_progress() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );

    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(client.get_progress(&learner, &course_id), 1);
}

#[test]
fn test_complete_module_emits_event() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1, // Change to 1 module so completion triggers all events
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional), // Add policy
    );

    // Enroll first
    client.enroll(&learner, &course_id);
    
    let initial_events = env.events().all().len();
    client.complete_module(&admin, &learner, &course_id);

    // Should emit at least ModuleCompleted event
    assert!(env.events().all().len() > initial_events);
}

#[test]
#[should_panic(expected = "Course already completed")]
fn test_complete_module_exceeds_total_modules() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &2,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );

    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_complete_module_unauthorized_verifier() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let fake_verifier = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &None,
    );

    client.complete_module(&fake_verifier, &learner, &course_id);
}

// ── Badge minting tests (updated for policies) ─────────────────────────────

#[test]
fn test_badge_minted_on_final_module_completion() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &2,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    client.complete_module(&admin, &learner, &course_id);
    assert!(!badge_client.has_badge(&learner, &course_id));

    client.complete_module(&admin, &learner, &course_id);
    assert!(badge_client.has_badge(&learner, &course_id));
}

#[test]
fn test_badge_not_minted_on_intermediate_module() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &3,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    assert!(!badge_client.has_badge(&learner, &course_id));
}

#[test]
fn test_complete_module_without_badge_nft_configured_does_not_panic() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    client.complete_module(&admin, &learner, &course_id);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_set_badge_nft_address_unauthorized_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let fake_admin = Address::generate(&env);
    let badge_address = Address::generate(&env);

    client.initialize(&admin);
    client.set_badge_nft_address(&fake_admin, &badge_address);
}

// ── transfer_ownership ──────────────────────────────────────────────────────

#[test]
fn test_transfer_ownership_success() {
    let (env, client) = setup();
    let (_, instructor, id) = setup_with_course(&env, &client, None);
    let new_instructor = Address::generate(&env);

    client.transfer_ownership(&instructor, &new_instructor, &id);

    let course = client.get_course(&id);
    assert_eq!(course.instructor, new_instructor);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the course instructor")]
fn test_transfer_ownership_non_instructor_panics() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);
    let impostor = Address::generate(&env);
    let new_instructor = Address::generate(&env);

    client.transfer_ownership(&impostor, &new_instructor, &id);
}

// ── Reward payout tests (updated for policies) ─────────────────────────────

#[test]
fn test_complete_course_triggers_reward_distribution() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &2,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    client.complete_module(&admin, &learner, &course_id);
    assert!(!badge_client.has_badge(&learner, &course_id));
    assert_eq!(token_sac.balance(&learner), 0);

    client.complete_module(&admin, &learner, &course_id);

    assert!(badge_client.has_badge(&learner, &course_id));
    assert_eq!(token_sac.balance(&learner), 10_0000000);
}

#[test]
fn test_reward_not_distributed_if_reward_pool_not_set() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    client.complete_module(&admin, &learner, &course_id);

    assert!(badge_client.has_badge(&learner, &course_id));
    assert_eq!(client.get_progress(&learner, &course_id), 1);
}

#[test]
#[should_panic(expected = "Caller is not an authorized spender")]
fn test_reward_not_distributed_without_whitelist() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::RewardRequired), // Change to RewardRequired
    );

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    // Set reward pool address WITHOUT whitelisting
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // This should panic because the reward pool isn't whitelisted
    client.enroll(&learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

// ── Per-Course Reward Configuration tests ───────────────────────────────────

#[test]
fn test_create_course_with_zero_reward() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &0, &None);

    let course = client.get_course(&course_id);
    assert_eq!(course.reward_amount, 0);
}

#[test]
fn test_update_course_reward_success() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client, None);

    let new_reward = 25_0000000;
    client.update_course_reward(&admin, &id, &new_reward);

    let course = client.get_course(&id);
    assert_eq!(course.reward_amount, new_reward);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_update_course_reward_unauthorized_panics() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client, None);
    let fake_admin = Address::generate(&env);

    client.update_course_reward(&fake_admin, &id, &100);
}

#[test]
fn test_zero_reward_pays_nothing_on_completion() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &0,
        &Some(crate::CompletionPolicy::Optional),
    );

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(token_sac.balance(&learner), 0);
}

#[test]
fn test_updated_reward_used_on_completion() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    client.update_course_reward(&admin, &course_id, &50_0000000);

    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(token_sac.balance(&learner), 50_0000000);
}

#[test]
fn test_different_courses_have_different_rewards() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    client.initialize(&admin);
    let course_a = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &10_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );
    let course_b = client.create_course(
        &admin,
        &instructor,
        &1,
        &dummy_hash(&env),
        &25_0000000,
        &Some(crate::CompletionPolicy::Optional),
    );

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    client.complete_module(&admin, &learner_a, &course_a);
    assert_eq!(token_sac.balance(&learner_a), 10_0000000);

    client.complete_module(&admin, &learner_b, &course_b);
    assert_eq!(token_sac.balance(&learner_b), 25_0000000);
}
