#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env,
};

use crate::{CourseRegistry, CourseRegistryClient, DataKey};
use badge_nft::{BadgeNFT, BadgeNFTClient};

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
fn setup_with_course(env: &Env, client: &CourseRegistryClient) -> (Address, Address, u32) {
    let admin = Address::generate(env);
    let instructor = Address::generate(env);
    client.initialize(&admin);
    let id = client.create_course(&admin, &instructor, &5, &dummy_hash(env), &10_0000000);
    (admin, instructor, id)
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_create_course_returns_id_one() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);

    let id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);
    assert_eq!(id, 1);
}

// ── create_course ─────────────────────────────────────────────────────────────

#[test]
fn test_course_count_increments() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.initialize(&admin);

    assert_eq!(client.course_count(), 0);
    client.create_course(&admin, &instructor, &2, &hash, &10_0000000);
    assert_eq!(client.course_count(), 1);
    client.create_course(&admin, &instructor, &5, &hash, &10_0000000);
    assert_eq!(client.course_count(), 2);
}

#[test]
#[should_panic(expected = "total_modules must be greater than 0")]
fn test_zero_modules_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &0, &dummy_hash(&env), &10_0000000);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_unauthorized_admin_panics() {
    let (env, client) = setup();
    let true_admin = Address::generate(&env);
    let fake_admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&true_admin);
    client.create_course(&fake_admin, &instructor, &3, &dummy_hash(&env), &10_0000000);
}

#[test]
fn test_course_created_event_emitted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &4, &dummy_hash(&env), &10_0000000);

    assert_eq!(env.events().all().len(), 1);
}

// ── update_metadata ───────────────────────────────────────────────────────────

#[test]
fn test_update_metadata_success() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
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
    let (_, _, id) = setup_with_course(&env, &client);
    let new_hash = BytesN::from_array(&env, &[2u8; 32]);

    client.update_metadata(&id, &new_hash);

    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_update_metadata_multiple_times() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
    let hash_v2 = BytesN::from_array(&env, &[2u8; 32]);
    let hash_v3 = BytesN::from_array(&env, &[3u8; 32]);

    client.update_metadata(&id, &hash_v2);
    client.update_metadata(&id, &hash_v3);
}

// ── enroll ────────────────────────────────────────────────────────────────────

#[test]
fn test_enroll_success() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);

    let learner = Address::generate(&env);

    // If this executes without panicking, the enrollment was successful
    client.enroll(&learner, &id);
}

#[test]
fn test_enroll_multiple_learners_same_course() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);

    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    // Both should succeed without panicking
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
    let id_1 = client.create_course(&admin, &instructor, &4, &hash, &10_0000000);
    let id_2 = client.create_course(&admin, &instructor, &8, &hash, &10_0000000);

    let learner = Address::generate(&env);

    // Learner should be able to enroll in both distinct courses
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
    let (_, _, id) = setup_with_course(&env, &client);

    let learner = Address::generate(&env);
    client.enroll(&learner, &id);

    // The second attempt must panic, proving the first enrollment was saved
    client.enroll(&learner, &id);
}

#[test]
fn test_create_and_get_course() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &5, &hash, &25_0000000);

    // Test: Retrieve the course using get_course
    let retrieved_course = client.get_course(&course_id);

    // Assert: Verify all fields match
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

    // Test: Try to retrieve a non-existent course
    let _ = client.get_course(&999);
}

#[test]
fn test_multiple_courses() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor1 = Address::generate(&env);
    let instructor2 = Address::generate(&env);
    let hash1 = dummy_hash(&env);
    let hash2 = BytesN::from_array(&env, &[2u8; 32]);

    client.initialize(&admin);
    let course_id1 = client.create_course(&admin, &instructor1, &10, &hash1, &10_0000000);
    let course_id2 = client.create_course(&admin, &instructor2, &7, &hash2, &10_0000000);

    // Test: Retrieve both courses
    let retrieved_course1 = client.get_course(&course_id1);
    let retrieved_course2 = client.get_course(&course_id2);

    // Assert: Verify each course is retrieved correctly
    assert_eq!(retrieved_course1.instructor, instructor1);
    assert_eq!(retrieved_course1.total_modules, 10);
    assert_eq!(retrieved_course2.instructor, instructor2);
    assert_eq!(retrieved_course2.total_modules, 7);
    assert_ne!(retrieved_course1.instructor, retrieved_course2.instructor);
}

// ── get_progress ─────────────────────────────────────────────────────────────

#[test]
fn test_get_progress_returns_zero_after_enroll() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
    let learner = Address::generate(&env);

    client.enroll(&learner, &id);

    let progress = client.get_progress(&learner, &id);
    assert_eq!(progress, 0);
}

#[test]
fn test_get_progress_returns_zero_when_unenrolled() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
    let learner = Address::generate(&env);

    // No enroll; call get_progress for unenrolled learner — must return 0 and not panic
    let progress = client.get_progress(&learner, &id);
    assert_eq!(progress, 0);
}

// ── is_course_finished tests ──────────────────────────────────────────────────

#[test]
fn test_is_course_finished_unenrolled_returns_false() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Learner has no progress entry at all — should return false
    assert!(!client.is_course_finished(&learner, &1));
}

#[test]
fn test_is_course_finished_partial_progress_returns_false() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &5, &dummy_hash(&env), &10_0000000);

    // Manually write partial progress into storage
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Progress(learner.clone(), 1), &3u32);
    });

    assert!(!client.is_course_finished(&learner, &1));
}

#[test]
fn test_is_course_finished_exact_progress_returns_true() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &4, &dummy_hash(&env), &10_0000000);

    // Progress exactly equals total_modules
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Progress(learner.clone(), 1), &4u32);
    });

    assert!(client.is_course_finished(&learner, &1));
}

#[test]
fn test_is_course_finished_excess_progress_returns_true() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Progress exceeds total_modules (defensive edge case)
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Progress(learner.clone(), 1), &99u32);
    });

    assert!(client.is_course_finished(&learner, &1));
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_is_course_finished_invalid_course_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);

    // Course ID 99 was never created
    client.is_course_finished(&learner, &99);
}

// ── set_course_status (Issue #4) ──────────────────────────────────────────────

#[test]
fn test_set_course_status_success() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client);

    // Deactivate the course
    client.set_course_status(&admin, &id, &false);

    // Verify it was deactivated
    let course = client.get_course(&id);
    assert!(!course.active);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_set_course_status_unauthorized_admin_panics() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
    let fake_admin = Address::generate(&env);

    // Random user tries to deactivate the course
    client.set_course_status(&fake_admin, &id, &false);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_set_course_status_nonexistent_course() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    client.initialize(&admin);
    client.set_course_status(&admin, &99, &false);
}

// ── complete_module Tests ─────────────────────────────────────────────────────

#[test]
fn test_complete_module_increments_progress() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Complete first module
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
fn test_complete_module_emits_event() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    client.complete_module(&admin, &learner, &course_id);

    // Verify event was emitted
    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_complete_module_multiple_times() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Complete all three modules
    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);
}

#[test]
#[should_panic(expected = "Course already completed")]
fn test_complete_module_exceeds_total_modules() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &2, &dummy_hash(&env), &10_0000000);

    // Complete both modules
    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    // This should panic - trying to complete a third module when only 2 exist
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
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Should fail - fake_verifier is not the admin
    client.complete_module(&fake_verifier, &learner, &course_id);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_complete_module_nonexistent_course() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);

    // Should fail - course 99 doesn't exist
    client.complete_module(&admin, &learner, &99);
}

#[test]
fn test_complete_module_different_learners() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Both learners can progress independently
    client.complete_module(&admin, &learner1, &course_id);
    client.complete_module(&admin, &learner2, &course_id);
    client.complete_module(&admin, &learner1, &course_id);
}

#[test]
fn test_get_progress_returns_zero_initially() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    // Progress should be 0 before any modules are completed
    assert_eq!(client.get_progress(&learner, &course_id), 0);
}

#[test]
fn test_get_progress_tracks_completion() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    assert_eq!(client.get_progress(&learner, &course_id), 0);

    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(client.get_progress(&learner, &course_id), 1);

    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(client.get_progress(&learner, &course_id), 2);

    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(client.get_progress(&learner, &course_id), 3);
}

// ── Badge minting on course completion ───────────────────────────────────────

/// Helper: deploys and initializes a BadgeNFT contract, authorizing the given registry address.
fn setup_badge_nft<'a>(env: &Env, registry_address: &Address) -> BadgeNFTClient<'a> {
    let badge_id = env.register(BadgeNFT, ());
    let badge_client = BadgeNFTClient::new(env, &badge_id);
    badge_client.initialize(registry_address);
    badge_client
}

#[test]
fn test_badge_minted_on_final_module_completion() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &2, &dummy_hash(&env), &10_0000000);

    // Deploy BadgeNFT and wire it up
    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Complete module 1 — no badge yet
    client.complete_module(&admin, &learner, &course_id);
    assert!(!badge_client.has_badge(&learner, &course_id));

    // Complete module 2 (final) — badge must be minted
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
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Complete only the first two of three modules
    client.complete_module(&admin, &learner, &course_id);
    client.complete_module(&admin, &learner, &course_id);

    assert!(!badge_client.has_badge(&learner, &course_id));
}

#[test]
fn test_badge_minted_for_multiple_learners_independently() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Each learner completes the single-module course
    client.complete_module(&admin, &learner_a, &course_id);
    client.complete_module(&admin, &learner_b, &course_id);

    assert!(badge_client.has_badge(&learner_a, &course_id));
    assert!(badge_client.has_badge(&learner_b, &course_id));
    assert_eq!(badge_client.get_badge_count(&learner_a), 1);
    assert_eq!(badge_client.get_badge_count(&learner_b), 1);
}

#[test]
fn test_badge_minted_for_different_courses_same_learner() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_a = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);
    let course_b = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    client.complete_module(&admin, &learner, &course_a);
    client.complete_module(&admin, &learner, &course_b);

    assert!(badge_client.has_badge(&learner, &course_a));
    assert!(badge_client.has_badge(&learner, &course_b));
    assert_eq!(badge_client.get_badge_count(&learner), 2);
}

#[test]
fn test_complete_module_without_badge_nft_configured_does_not_panic() {
    // If set_badge_nft_address was never called, completion should still succeed
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    // No badge NFT address set — final module completion must not panic
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

// ── transfer_ownership ────────────────────────────────────────────────────────

#[test]
fn test_transfer_ownership_success() {
    let (env, client) = setup();
    let (_, instructor, id) = setup_with_course(&env, &client);
    let new_instructor = Address::generate(&env);

    client.transfer_ownership(&instructor, &new_instructor, &id);

    let course = client.get_course(&id);
    assert_eq!(course.instructor, new_instructor);
}

#[test]
fn test_transfer_ownership_emits_event() {
    let (env, client) = setup();
    let (_, instructor, id) = setup_with_course(&env, &client);
    let new_instructor = Address::generate(&env);

    client.transfer_ownership(&instructor, &new_instructor, &id);

    // One OwnershipTransferred event must have been emitted
    assert_eq!(env.events().all().len(), 1);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the course instructor")]
fn test_transfer_ownership_non_instructor_panics() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
    let impostor = Address::generate(&env);
    let new_instructor = Address::generate(&env);

    client.transfer_ownership(&impostor, &new_instructor, &id);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_transfer_ownership_nonexistent_course_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let new_instructor = Address::generate(&env);

    client.initialize(&admin);

    client.transfer_ownership(&instructor, &new_instructor, &99);
}

#[test]
fn test_transfer_ownership_new_instructor_can_update_metadata() {
    let (env, client) = setup();
    let (_, instructor, id) = setup_with_course(&env, &client);
    let new_instructor = Address::generate(&env);

    client.transfer_ownership(&instructor, &new_instructor, &id);

    // New instructor must be able to update metadata after ownership transfer
    let updated_hash = BytesN::from_array(&env, &[9u8; 32]);
    client.update_metadata(&id, &updated_hash);

    let course = client.get_course(&id);
    assert_eq!(course.metadata_hash, updated_hash);
}

#[test]
fn test_transfer_ownership_updates_instructor_field() {
    let (env, client) = setup();
    let (_, instructor, id) = setup_with_course(&env, &client);
    let new_instructor = Address::generate(&env);

    // Confirm original instructor before transfer
    let before = client.get_course(&id);
    assert_eq!(before.instructor, instructor);

    client.transfer_ownership(&instructor, &new_instructor, &id);

    // Confirm instructor field reflects the new address after transfer
    let after = client.get_course(&id);
    assert_eq!(after.instructor, new_instructor);
    assert_ne!(after.instructor, instructor);
}

// ── Reward payout on course completion (Issue #53) ────────────────────────────

use reward_pool::{RewardPool, RewardPoolClient};
use soroban_sdk::token;

/// Deploys + initializes a RewardPool backed by a real SAC token.
/// Returns (reward_pool_client, token_admin, token_sac_client, token_address).
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

/// Test 1 – Complete course triggers reward distribution (full happy path).
#[test]
fn test_complete_course_triggers_reward_distribution() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    // Initialise CourseRegistry and create a 2-module course
    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &2, &dummy_hash(&env), &10_0000000);

    // Deploy RewardPool and fund it
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000); // 100 USDC

    // Wire up: whitelist CourseRegistry in RewardPool, set RewardPool address in CourseRegistry
    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Also wire up a badge NFT so we can confirm badge + reward both fire
    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Module 1 — no reward yet
    client.complete_module(&admin, &learner, &course_id);
    assert!(!badge_client.has_badge(&learner, &course_id));
    assert_eq!(token_sac.balance(&learner), 0);

    // Module 2 (final) — badge minted AND reward transferred
    client.complete_module(&admin, &learner, &course_id);

    // Verify CourseCompleted event was emitted immediately after contract call
    // (subsequent client calls like has_badge or balance will clear the event log)
    let all_events = env.events().all();
    assert!(!all_events.is_empty());

    assert!(badge_client.has_badge(&learner, &course_id));
    assert_eq!(token_sac.balance(&learner), 10_0000000); // 10 USDC
}

/// Test 2 – Reward NOT distributed when CourseRegistry is not whitelisted.
#[test]
#[should_panic(expected = "Caller is not an authorized spender")]
fn test_reward_not_distributed_without_whitelist() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    // Deploy RewardPool and fund it — but do NOT whitelist CourseRegistry
    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    // Set the RewardPool address WITHOUT calling add_approved_spender
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Should panic: "Caller is not an authorized spender"
    client.complete_module(&admin, &learner, &course_id);
}

/// Test 3 – No reward distributed if RewardPool address was never set (graceful degradation).
#[test]
fn test_reward_not_distributed_if_reward_pool_not_set() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    // Wire badge NFT but deliberately omit set_reward_pool_address
    let badge_client = setup_badge_nft(&env, &client.address);
    client.set_badge_nft_address(&admin, &badge_client.address);

    // Completing the only module must NOT panic
    client.complete_module(&admin, &learner, &course_id);

    // Badge is still minted
    assert!(badge_client.has_badge(&learner, &course_id));
    // Progress reached total_modules
    assert_eq!(client.get_progress(&learner, &course_id), 1);
}

/// Test 4 – Multiple learners each receive independent rewards.
#[test]
fn test_multiple_learners_get_independent_rewards() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000); // enough for both

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Learner A completes the course
    client.complete_module(&admin, &learner_a, &course_id);
    assert_eq!(token_sac.balance(&learner_a), 10_0000000);

    // Learner B completes the course
    client.complete_module(&admin, &learner_b, &course_id);
    assert_eq!(token_sac.balance(&learner_b), 10_0000000);

    // Pool balance decreased by 2 × 10 USDC
    assert_eq!(
        token_sac.balance(&reward_pool_client.address),
        1_000_000_000 - 2 * 10_0000000
    );
}

/// Test 5 – Reward is distributed ONLY on the final module (not intermediate ones).
#[test]
fn test_reward_distributed_only_on_final_module() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &10_0000000);

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Module 1 — no reward
    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(token_sac.balance(&learner), 0);

    // Module 2 — no reward
    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(token_sac.balance(&learner), 0);

    // Module 3 (final) — reward paid out
    client.complete_module(&admin, &learner, &course_id);
    assert_eq!(token_sac.balance(&learner), 10_0000000);
}

// ── Per-Course Reward Configuration (Issue #95) ─────────────────────────────

#[test]
fn test_create_course_with_zero_reward() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &0);

    let course = client.get_course(&course_id);
    assert_eq!(course.reward_amount, 0);
}

#[test]
fn test_create_course_with_positive_reward() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &5, &dummy_hash(&env), &50_0000000);

    let course = client.get_course(&course_id);
    assert_eq!(course.reward_amount, 50_0000000);
}

#[test]
#[should_panic(expected = "reward_amount must be non-negative")]
fn test_create_course_with_negative_reward_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &3, &dummy_hash(&env), &-1);
}

#[test]
fn test_update_course_reward_success() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client);

    let new_reward = 25_0000000;
    client.update_course_reward(&admin, &id, &new_reward);

    let course = client.get_course(&id);
    assert_eq!(course.reward_amount, new_reward);
}

#[test]
fn test_update_course_reward_emits_event() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client);

    client.update_course_reward(&admin, &id, &25_0000000);

    let all_events = env.events().all();
    assert!(!all_events.is_empty());
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_update_course_reward_unauthorized_panics() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);
    let fake_admin = Address::generate(&env);

    client.update_course_reward(&fake_admin, &id, &100);
}

#[test]
#[should_panic(expected = "reward_amount must be non-negative")]
fn test_update_course_reward_negative_panics() {
    let (env, client) = setup();
    let (admin, _, id) = setup_with_course(&env, &client);

    client.update_course_reward(&admin, &id, &-1);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_update_course_reward_nonexistent_course_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    client.initialize(&admin);
    client.update_course_reward(&admin, &99, &100);
}

#[test]
fn test_zero_reward_pays_nothing_on_completion() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&admin);
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &0);

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
    let course_id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Update reward before completion
    client.update_course_reward(&admin, &course_id, &50_0000000);

    // Complete the course - should use the updated reward
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
    let course_a = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &10_0000000);
    let course_b = client.create_course(&admin, &instructor, &1, &dummy_hash(&env), &25_0000000);

    let (reward_pool_client, token_sac, _) = setup_reward_pool(&env, &admin);
    token_sac.mint(&reward_pool_client.address, &1_000_000_000);

    reward_pool_client.add_approved_spender(&admin, &client.address);
    client.set_reward_pool_address(&admin, &reward_pool_client.address);

    // Learner A completes course_a (10 USDC reward)
    client.complete_module(&admin, &learner_a, &course_a);
    assert_eq!(token_sac.balance(&learner_a), 10_0000000);

    // Learner B completes course_b (25 USDC reward)
    client.complete_module(&admin, &learner_b, &course_b);
    assert_eq!(token_sac.balance(&learner_b), 25_0000000);
}
