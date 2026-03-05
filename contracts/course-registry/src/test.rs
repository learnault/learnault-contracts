use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(crate::CourseRegistry, ());
    (env, contract_id)
}

// --- enroll: happy path ---

#[test]
fn test_enroll_initializes_progress_to_zero() {
    let (env, contract_id) = setup();
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    client.create_course(&1u32, &5u32);

    let learner = Address::generate(&env);
    client.enroll(&learner, &1u32);

    assert_eq!(client.get_progress(&learner, &1u32), 0u32);
}

#[test]
fn test_enroll_multiple_learners_same_course() {
    let (env, contract_id) = setup();
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    client.create_course(&1u32, &5u32);

    let learner_a = Address::generate(&env);
    let learner_b = Address::generate(&env);

    client.enroll(&learner_a, &1u32);
    client.enroll(&learner_b, &1u32);

    assert_eq!(client.get_progress(&learner_a, &1u32), 0u32);
    assert_eq!(client.get_progress(&learner_b, &1u32), 0u32);
}

#[test]
fn test_enroll_same_learner_different_courses() {
    let (env, contract_id) = setup();
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    client.create_course(&1u32, &4u32);
    client.create_course(&2u32, &8u32);

    let learner = Address::generate(&env);

    client.enroll(&learner, &1u32);
    client.enroll(&learner, &2u32);

    assert_eq!(client.get_progress(&learner, &1u32), 0u32);
    assert_eq!(client.get_progress(&learner, &2u32), 0u32);
}

// --- enroll: error paths ---

#[test]
#[should_panic]
fn test_enroll_panics_when_course_does_not_exist() {
    let (env, contract_id) = setup();
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    let learner = Address::generate(&env);
    client.enroll(&learner, &99u32);
}

#[test]
#[should_panic]
fn test_enroll_panics_when_course_is_inactive() {
    let (env, contract_id) = setup();
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    client.create_course(&1u32, &5u32);
    client.set_active(&1u32, &false);

    let learner = Address::generate(&env);
    client.enroll(&learner, &1u32);
}

#[test]
#[should_panic]
fn test_enroll_panics_when_learner_already_enrolled() {
    let (env, contract_id) = setup();
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    client.create_course(&1u32, &5u32);

    let learner = Address::generate(&env);
    client.enroll(&learner, &1u32);
    client.enroll(&learner, &1u32); // second enroll must panic
}
