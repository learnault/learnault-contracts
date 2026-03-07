#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    token, Address, Env,
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

    client.initialize(&admin, &token);
    client.add_approved_spender(&admin, &spender);
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_add_approved_spender_not_initialized() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender = Address::generate(&env);

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

    client.initialize(&admin, &token);
    client.add_approved_spender(&wrong_admin, &spender);
}

// ── distribute_reward Tests ───────────────────────────────────────────────────

#[test]
fn test_distribute_reward_success() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender = Address::generate(&env);
    let learner = Address::generate(&env);
    
    // Create and register a mock token contract
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    
    // Initialize the reward pool
    client.initialize(&admin, &token_id.address());
    
    // Whitelist the spender
    client.add_approved_spender(&admin, &spender);
    
    // Mint tokens to the reward pool contract
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&client.address, &1000);
    
    // Distribute reward
    client.distribute_reward(&spender, &learner, &100);
    
    // Verify learner received tokens
    let learner_balance = token_client.balance(&learner);
    assert_eq!(learner_balance, 100);
}

#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_distribute_reward_zero_amount() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender = Address::generate(&env);
    let learner = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    
    client.initialize(&admin, &token_id.address());
    client.add_approved_spender(&admin, &spender);
    
    // Try to distribute zero amount - should panic
    client.distribute_reward(&spender, &learner, &0);
}

#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_distribute_reward_negative_amount() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender = Address::generate(&env);
    let learner = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    
    client.initialize(&admin, &token_id.address());
    client.add_approved_spender(&admin, &spender);
    
    // Try to distribute negative amount - should panic
    client.distribute_reward(&spender, &learner, &-100);
}

#[test]
#[should_panic(expected = "Caller is not an authorized spender")]
fn test_distribute_reward_unauthorized_spender() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let unauthorized_spender = Address::generate(&env);
    let learner = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    
    client.initialize(&admin, &token_id.address());
    
    // Try to distribute without being whitelisted - should panic
    client.distribute_reward(&unauthorized_spender, &learner, &100);
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_distribute_reward_not_initialized() {
    let (env, client) = setup();
    let spender = Address::generate(&env);
    let learner = Address::generate(&env);
    
    // Try to distribute without initializing - should panic
    client.distribute_reward(&spender, &learner, &100);
}

#[test]
fn test_distribute_reward_multiple_times() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender = Address::generate(&env);
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    
    client.initialize(&admin, &token_id.address());
    client.add_approved_spender(&admin, &spender);
    
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&client.address, &1000);
    
    // Distribute to multiple learners
    client.distribute_reward(&spender, &learner1, &100);
    client.distribute_reward(&spender, &learner2, &200);
    
    assert_eq!(token_client.balance(&learner1), 100);
    assert_eq!(token_client.balance(&learner2), 200);
}

#[test]
fn test_distribute_reward_multiple_spenders() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let spender1 = Address::generate(&env);
    let spender2 = Address::generate(&env);
    let learner = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    
    client.initialize(&admin, &token_id.address());
    client.add_approved_spender(&admin, &spender1);
    client.add_approved_spender(&admin, &spender2);
    
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&client.address, &1000);
    
    // Both spenders can distribute
    client.distribute_reward(&spender1, &learner, &100);
    client.distribute_reward(&spender2, &learner, &50);
    
    assert_eq!(token_client.balance(&learner), 150);
}
