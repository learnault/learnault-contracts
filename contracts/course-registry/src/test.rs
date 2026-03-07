#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env,
};

use crate::{CourseRegistry, CourseRegistryClient, DataKey};

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
    let id = client.create_course(&admin, &instructor, &5, &dummy_hash(env));
    (admin, instructor, id)
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_create_course_returns_id_one() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);

    let id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env));
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
    client.create_course(&admin, &instructor, &2, &hash);
    assert_eq!(client.course_count(), 1);
    client.create_course(&admin, &instructor, &5, &hash);
    assert_eq!(client.course_count(), 2);
}

#[test]
#[should_panic(expected = "total_modules must be greater than 0")]
fn test_zero_modules_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &0, &dummy_hash(&env));
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the protocol admin")]
fn test_unauthorized_admin_panics() {
    let (env, client) = setup();
    let true_admin = Address::generate(&env);
    let fake_admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&true_admin);
    client.create_course(&fake_admin, &instructor, &3, &dummy_hash(&env));
}

#[test]
fn test_course_created_event_emitted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &4, &dummy_hash(&env));

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
    let id_1 = client.create_course(&admin, &instructor, &4, &hash);
    let id_2 = client.create_course(&admin, &instructor, &8, &hash);

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
    let course_id = client.create_course(&admin, &instructor, &5, &hash);

    // Test: Retrieve the course using get_course
    let retrieved_course = client.get_course(&course_id);

    // Assert: Verify all fields match
    assert_eq!(retrieved_course.instructor, instructor);
    assert_eq!(retrieved_course.total_modules, 5);
    assert_eq!(retrieved_course.metadata_hash, hash);
    assert!(retrieved_course.active);
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
    let course_id1 = client.create_course(&admin, &instructor1, &10, &hash1);
    let course_id2 = client.create_course(&admin, &instructor2, &7, &hash2);

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
    client.create_course(&admin, &instructor, &3, &dummy_hash(&env));

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
    client.create_course(&admin, &instructor, &5, &dummy_hash(&env));

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
    client.create_course(&admin, &instructor, &4, &dummy_hash(&env));

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
    client.create_course(&admin, &instructor, &3, &dummy_hash(&env));

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
