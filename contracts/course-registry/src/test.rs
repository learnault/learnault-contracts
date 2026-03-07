#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env,
};

use crate::{CourseRegistry, CourseRegistryClient};

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
) -> (Address, Address, u32) {
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
fn test_enroll_initializes_progress_to_zero() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);

    let learner = Address::generate(&env);
    client.enroll(&learner, &id);

    assert_eq!(client.get_progress(&learner, &id), 0u32);
}

#[test]
fn test_enroll_multiple_learners_same_course() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);

    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    client.enroll(&learner_a, &id);
    client.enroll(&learner_b, &id);

    assert_eq!(client.get_progress(&learner_a, &id), 0u32);
    assert_eq!(client.get_progress(&learner_b, &id), 0u32);
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
    client.enroll(&learner, &id_1);
    client.enroll(&learner, &id_2);

    assert_eq!(client.get_progress(&learner, &id_1), 0u32);
    assert_eq!(client.get_progress(&learner, &id_2), 0u32);
}

#[test]
#[should_panic]
fn test_enroll_panics_when_course_does_not_exist() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let learner = Address::generate(&env);
    client.enroll(&learner, &99u32);
}

#[test]
#[should_panic]
fn test_enroll_panics_when_course_is_inactive() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);
    let id = client.create_course(&admin, &instructor, &5, &dummy_hash(&env));

    // Deactivate by updating with a course that has active: false
    // Since set_active was removed, directly test via a non-active state isn't
    // possible without re-adding it — leave as compile-time reminder.
    // TODO: re-add set_active or test deactivation once the function is restored.
    let learner = Address::generate(&env);
    client.enroll(&learner, &id);
}

#[test]
#[should_panic]
fn test_enroll_panics_when_learner_already_enrolled() {
    let (env, client) = setup();
    let (_, _, id) = setup_with_course(&env, &client);

    let learner = Address::generate(&env);
    client.enroll(&learner, &id);
    client.enroll(&learner, &id); // must panic
}