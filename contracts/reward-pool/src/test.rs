#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, Map, Symbol,
};

use crate::{RewardPool, RewardPoolClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, RewardPoolClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    // Fixed: Passing the contract type first, and empty constructor args second
    let contract_id = env.register(RewardPool, ());

    let client = RewardPoolClient::new(&env, &contract_id);
    (env, client)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize the contract
    client.initialize(&admin, &token);

    // Verify event was emitted
    assert_eq!(env.events().all().len(), 1);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // First initialization should succeed
    client.initialize(&admin, &token);

    // Second initialization should panic
    client.initialize(&admin, &token);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_initialize_without_auth_panics() {
    let env = Env::default();
    let contract_id = env.register(RewardPool, ());
    let client = RewardPoolClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Try to initialize without mocking auths - should panic
    client.initialize(&admin, &token);
}

#[test]
fn test_initialize_with_proper_auth() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize with proper authentication (mocked by env.mock_all_auths())
    client.initialize(&admin, &token);

    // Verify event was emitted
    assert_eq!(env.events().all().len(), 1);
}

// ── add_approved_spender Tests ────────────────────────────────────────────────

#[test]
fn test_add_approved_spender_success() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let spender = Address::generate(&env);

    // Initialize the contract
    client.initialize(&admin, &token);

    // Add approved spender - should succeed without panic
    client.add_approved_spender(&admin, &spender);

    // assert event emitted
    let empty_data: Map<(), ()> = Map::new(&env);
    let event = vec![
        &env,
        (
            client.address,
            (Symbol::new(&env, "spender_added"), spender).into_val(&env),
            empty_data.into_val(&env),
        ),
    ];

    assert_eq!(env.events().all(), event)
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_add_approved_spender_not_initialized() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender = Address::generate(&env);

    // Try to add spender without initializing - should panic
    client.add_approved_spender(&admin, &spender);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_add_approved_spender_wrong_admin() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let wrong_admin = Address::generate(&env);
    let token = Address::generate(&env);
    let spender = Address::generate(&env);

    // Initialize the contract
    client.initialize(&admin, &token);

    // Try to add spender with wrong admin - should panic
    client.add_approved_spender(&wrong_admin, &spender);
}

#[test]
fn test_add_multiple_approved_spenders() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let spender1 = Address::generate(&env);
    let spender2 = Address::generate(&env);

    // Initialize the contract
    client.initialize(&admin, &token);

    // Add multiple spenders - should succeed without panic
    client.add_approved_spender(&admin, &spender1);
    client.add_approved_spender(&admin, &spender2);
}

#[test]
fn test_add_same_spender_twice() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let spender = Address::generate(&env);

    // Initialize the contract
    client.initialize(&admin, &token);

    // Add same spender twice (should not panic, just overwrite)
    client.add_approved_spender(&admin, &spender);
    client.add_approved_spender(&admin, &spender);
}
