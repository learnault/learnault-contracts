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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_create_course_returns_id_one() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    let id = client.create_course(&admin, &instructor, &3, &dummy_hash(&env));
    assert_eq!(id, 1);
}

#[test]
fn test_course_count_increments() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    assert_eq!(client.course_count(), 0);
    client.create_course(&admin, &instructor, &2, &hash);
    assert_eq!(client.course_count(), 1);
    client.create_course(&admin, &instructor, &5, &hash);
    assert_eq!(client.course_count(), 2);
}

#[test]
fn test_multiple_courses_have_sequential_ids() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    let id1 = client.create_course(&admin, &instructor, &1, &hash);
    let id2 = client.create_course(&admin, &instructor, &1, &hash);
    let id3 = client.create_course(&admin, &instructor, &1, &hash);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_get_course_returns_correct_data() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    let id = client.create_course(&admin, &instructor, &7, &hash);
    let course = client.get_course(&id);

    assert_eq!(course.id, id);
    assert_eq!(course.instructor, instructor);
    assert_eq!(course.total_modules, 7);
    assert_eq!(course.metadata_hash, hash);
    assert!(course.active);
}

#[test]
fn test_course_defaults_to_active() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);

    let id = client.create_course(&admin, &instructor, &1, &dummy_hash(&env));
    let course = client.get_course(&id);
    assert!(course.active);
}

#[test]
#[should_panic(expected = "total_modules must be greater than 0")]
fn test_zero_modules_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    client.create_course(&admin, &instructor, &0, &dummy_hash(&env));
}

#[test]
fn test_course_created_event_emitted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let instructor = Address::generate(&env);
    let hash = dummy_hash(&env);

    client.create_course(&admin, &instructor, &4, &hash);

    // Verify exactly one contract event was emitted.
    assert_eq!(env.events().all().len(), 1);
}

#[test]
#[should_panic(expected = "course not found")]
fn test_get_nonexistent_course_panics() {
    let (_env, client) = setup();
    client.get_course(&999);
}
