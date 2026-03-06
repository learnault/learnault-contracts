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

    // Fixed: Passing the contract type first, and empty constructor args second
    let contract_id = env.register(CourseRegistry, ());

    let client = CourseRegistryClient::new(&env, &contract_id);
    (env, client)
}

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_create_course_returns_id_one() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    client.initialize(&admin);

    let id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env));
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

    // Fails because fake_admin does not match true_admin
    client.create_course(&fake_admin, &instructor, &3, &dummy_hash(&env));
}

#[test]
fn test_course_created_event_emitted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &4, &hash);

    // Verify exactly one contract event was published via the macro.
    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_update_metadata_success() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);
    let new_hash = BytesN::from_array(&env, &[2u8; 32]);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &3, &hash);
    client.update_metadata(&1, &new_hash);
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
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);
    let new_hash = BytesN::from_array(&env, &[2u8; 32]);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &3, &hash);
    client.update_metadata(&1, &new_hash);

    // events().all() returns events from the most recent invocation
    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_update_metadata_multiple_times() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);
    let hash_v2 = BytesN::from_array(&env, &[2u8; 32]);
    let hash_v3 = BytesN::from_array(&env, &[3u8; 32]);

    client.initialize(&admin);
    client.create_course(&admin, &instructor, &3, &hash);
    client.update_metadata(&1, &hash_v2);
    client.update_metadata(&1, &hash_v3);
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
