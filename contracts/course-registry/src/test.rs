use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{CourseRegistry, CourseRegistryClient, DataKey};

#[test]
fn get_progress_returns_zero_when_unenrolled() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CourseRegistry);
    let client = CourseRegistryClient::new(&env, &contract_id);

    let learner = Address::generate(&env);
    let course_id = 1u32;

    let progress = client.get_progress(&learner, &course_id);
    assert_eq!(progress, 0);
}

#[test]
fn get_progress_returns_stored_value_when_enrolled() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CourseRegistry);
    let client = CourseRegistryClient::new(&env, &contract_id);

    let learner = Address::generate(&env);
    let course_id = 2u32;
    let completed_modules = 5u32;

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Progress(learner.clone(), course_id), &completed_modules);
    });

    let progress = client.get_progress(&learner, &course_id);
    assert_eq!(progress, completed_modules);
}
