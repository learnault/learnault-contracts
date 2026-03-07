#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String,
};

use crate::{RewardPool, RewardPoolClient};
use soroban_sdk::token;
use token::{Token, TokenClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, RewardPoolClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

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

    client.initialize(&admin, &token);

    assert_eq!(env.events().all().len(), 1);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_panics() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);

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

    client.initialize(&admin, &token);
}

#[test]
fn test_initialize_with_proper_auth() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);

    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_fund_pool_transfers_balance() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let donor = Address::generate(&env);

    let token_id = env.register(Token, ());
    let token_client = TokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &6u32,
        &String::from_str(&env, "USDC"),
        &String::from_str(&env, "USDC"),
    );

    client.initialize(&admin, &token_id);

    token_client.mint(&donor, &1_000i128);

    let sac = token::Client::new(&env, &token_id);
    let pool = env.current_contract_address();

    let donor_before = sac.balance(&donor);
    let pool_before = sac.balance(&pool);

    client.fund_pool(&donor, &100i128);

    let donor_after = sac.balance(&donor);
    let pool_after = sac.balance(&pool);

    assert_eq!(donor_before - 100, donor_after);
    assert_eq!(pool_before + 100, pool_after);
}

#[test]
fn test_fund_pool_emits_event() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let donor = Address::generate(&env);

    let token_id = env.register(Token, ());
    let token_client = TokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &6u32,
        &String::from_str(&env, "USDC"),
        &String::from_str(&env, "USDC"),
    );

    client.initialize(&admin, &token_id);
    token_client.mint(&donor, &500i128);

    let before = env.events().all().len();
    client.fund_pool(&donor, &200i128);
    let after = env.events().all().len();

    assert_eq!(after, before + 1);
}
