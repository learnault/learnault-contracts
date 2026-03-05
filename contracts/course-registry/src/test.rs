use crate::{CourseRegistry, CourseRegistryClient};
use soroban_sdk::{testutils::{Address as _, Events}, Address, BytesN, Env};

/// Helper function to create a test environment and register the contract.
fn setup_test_env() -> (Env, CourseRegistryClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CourseRegistry, ());
    let client = CourseRegistryClient::new(&env, &contract_id);

    let instructor = Address::generate(&env);
    let other_user = Address::generate(&env);

    (env, client, instructor, other_user)
}

/// Helper function to create a test hash.
fn create_test_hash(env: &Env, value: u8) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = value;
    BytesN::from_array(env, &bytes)
}

#[test]
fn test_create_and_get_course() {
    let (env, client, instructor, _) = setup_test_env();

    let course_id = 1;
    let metadata_hash = create_test_hash(&env, 1);

    // Create a course
    client.create_course(&course_id, &instructor, &metadata_hash);

    // Retrieve the course
    let course = client.get_course(&course_id);

    assert_eq!(course.instructor, instructor);
    assert_eq!(course.metadata_hash, metadata_hash);
}

#[test]
fn test_update_metadata_success() {
    let (env, client, instructor, _) = setup_test_env();

    let course_id = 1;
    let initial_hash = create_test_hash(&env, 1);
    let new_hash = create_test_hash(&env, 2);

    // Create a course
    client.create_course(&course_id, &instructor, &initial_hash);

    // Verify initial state
    let course_before = client.get_course(&course_id);
    assert_eq!(course_before.metadata_hash, initial_hash);

    // Update metadata
    client.update_metadata(&course_id, &new_hash);

    // Verify the hash was updated
    let course_after = client.get_course(&course_id);
    assert_eq!(course_after.metadata_hash, new_hash);
    assert_eq!(course_after.instructor, instructor); // Instructor should remain the same
}

#[test]
fn test_update_metadata_emits_event() {
    let (env, client, instructor, _) = setup_test_env();

    let course_id = 1;
    let initial_hash = create_test_hash(&env, 1);
    let new_hash = create_test_hash(&env, 2);

    // Create a course
    client.create_course(&course_id, &instructor, &initial_hash);

    // Update metadata
    client.update_metadata(&course_id, &new_hash);

    // Verify event was emitted
    let events = env.events().all();
    assert!(!events.is_empty(), "No events were emitted");

    // Note: In a real test, you would parse and verify the event structure
    // For now, we just verify that events were emitted
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_update_nonexistent_course() {
    let (env, client, _, _) = setup_test_env();

    let course_id = 999; // Non-existent course
    let new_hash = create_test_hash(&env, 2);

    // Attempt to update a non-existent course - should panic
    client.update_metadata(&course_id, &new_hash);
}

#[test]
#[should_panic(expected = "Course already exists")]
fn test_create_duplicate_course() {
    let (env, client, instructor, _) = setup_test_env();

    let course_id = 1;
    let metadata_hash = create_test_hash(&env, 1);

    // Create a course
    client.create_course(&course_id, &instructor, &metadata_hash);

    // Attempt to create the same course again - should panic
    client.create_course(&course_id, &instructor, &metadata_hash);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_get_nonexistent_course() {
    let (_, client, _, _) = setup_test_env();

    let course_id = 999; // Non-existent course

    // Attempt to get a non-existent course - should panic
    client.get_course(&course_id);
}

#[test]
fn test_multiple_courses() {
    let (env, client, instructor1, instructor2) = setup_test_env();

    let course_id_1 = 1;
    let course_id_2 = 2;
    let hash_1 = create_test_hash(&env, 1);
    let hash_2 = create_test_hash(&env, 2);

    // Create two different courses
    client.create_course(&course_id_1, &instructor1, &hash_1);
    client.create_course(&course_id_2, &instructor2, &hash_2);

    // Verify both courses exist and are independent
    let course1 = client.get_course(&course_id_1);
    let course2 = client.get_course(&course_id_2);

    assert_eq!(course1.instructor, instructor1);
    assert_eq!(course1.metadata_hash, hash_1);
    assert_eq!(course2.instructor, instructor2);
    assert_eq!(course2.metadata_hash, hash_2);
}

#[test]
fn test_update_metadata_multiple_times() {
    let (env, client, instructor, _) = setup_test_env();

    let course_id = 1;
    let hash_1 = create_test_hash(&env, 1);
    let hash_2 = create_test_hash(&env, 2);
    let hash_3 = create_test_hash(&env, 3);

    // Create a course
    client.create_course(&course_id, &instructor, &hash_1);

    // Update metadata multiple times
    client.update_metadata(&course_id, &hash_2);
    let course = client.get_course(&course_id);
    assert_eq!(course.metadata_hash, hash_2);

    client.update_metadata(&course_id, &hash_3);
    let course = client.get_course(&course_id);
    assert_eq!(course.metadata_hash, hash_3);
}
