use soroban_sdk::{symbol_short, Address, Env};

#[test]
fn test_create_course() {
    let env = Env::default();
    let contract_id = env.register(crate::CourseRegistry, ());
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    let admin = Address::from_str(
        &env,
        "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
    );

    let result = client.create_course(&admin, &1u32, &symbol_short!("Rust101"));
    assert_eq!(result, symbol_short!("success"));

    // Verify course was created
    let (id, title, active) = client.get_course(&1u32);
    assert_eq!(id, 1u32);
    assert_eq!(title, symbol_short!("Rust101"));
    assert!(active);
}

#[test]
fn test_deactivate_course() {
    let env = Env::default();
    let contract_id = env.register(crate::CourseRegistry, ());
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    let admin = Address::from_str(
        &env,
        "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
    );

    // Create a course
    client.create_course(&admin, &1u32, &symbol_short!("Rust101"));

    // Verify course is active
    let (_, _, active) = client.get_course(&1u32);
    assert!(active);

    // Deactivate the course
    let result = client.set_course_status(&admin, &1u32, &false);
    assert_eq!(result, symbol_short!("success"));

    // Verify course is now deactivated
    let (_, _, active) = client.get_course(&1u32);
    assert!(!active);
}

#[test]
fn test_reactivate_course() {
    let env = Env::default();
    let contract_id = env.register(crate::CourseRegistry, ());
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    let admin = Address::from_str(
        &env,
        "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
    );

    // Create and deactivate a course
    client.create_course(&admin, &1u32, &symbol_short!("Rust101"));
    client.set_course_status(&admin, &1u32, &false);

    // Verify course is deactivated
    let (_, _, active) = client.get_course(&1u32);
    assert!(!active);

    // Reactivate the course
    let result = client.set_course_status(&admin, &1u32, &true);
    assert_eq!(result, symbol_short!("success"));

    // Verify course is now active
    let (_, _, active) = client.get_course(&1u32);
    assert!(active);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_set_status_nonexistent_course() {
    let env = Env::default();
    let contract_id = env.register(crate::CourseRegistry, ());
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    let admin = Address::from_str(
        &env,
        "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
    );

    // Try to deactivate a course that doesn't exist
    client.set_course_status(&admin, &999u32, &false);
}

#[test]
#[should_panic(expected = "Course not found")]
fn test_get_nonexistent_course() {
    let env = Env::default();
    let contract_id = env.register(crate::CourseRegistry, ());
    let client = crate::CourseRegistryClient::new(&env, &contract_id);

    // Try to get a course that doesn't exist
    client.get_course(&999u32);
}
