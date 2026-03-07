#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events},
    Address, Env, String,
};

use crate::{RewardPool, RewardPoolClient};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum TTKey {
    Admin,
    Decimals,
    Name,
    Symbol,
}

#[contract]
struct TestToken;

#[contractimpl]
impl TestToken {
    pub fn initialize(env: Env, admin: Address, decimals: u32, name: String, symbol: String) {
        admin.require_auth();
        env.storage().instance().set(&TTKey::Admin, &admin);
        env.storage().instance().set(&TTKey::Decimals, &decimals);
        env.storage().instance().set(&TTKey::Name, &name);
        env.storage().instance().set(&TTKey::Symbol, &symbol);
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&TTKey::Admin)
            .expect("Token not initialized");
        admin.require_auth();
        let bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        env.storage().persistent().set(&to, &(bal + amount));
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().persistent().get(&id).unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        if amount < 0 {
            panic!("invalid amount");
        }
        let from_bal: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        if from_bal < amount {
            panic!("insufficient balance");
        }
        let to_bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        env.storage().persistent().set(&from, &(from_bal - amount));
        env.storage().persistent().set(&to, &(to_bal + amount));
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, RewardPoolClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(RewardPool, ());

    let client = RewardPoolClient::new(&env, &contract_id);
    (env, client, contract_id)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let (env, client, _) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);

    assert_eq!(env.events().all().len(), 1);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_panics() {
    let (env, client, _) = setup();
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
    let (env, client, _) = setup();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);

    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn test_fund_pool_transfers_balance() {
    let (env, client, pool_addr) = setup();

    let admin = Address::generate(&env);
    let donor = Address::generate(&env);

    let token_id = env.register(TestToken, ());
    let token_client = TestTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &6u32,
        &String::from_str(&env, "USDC"),
        &String::from_str(&env, "USDC"),
    );

    client.initialize(&admin, &token_id);

    token_client.mint(&donor, &1_000i128);

    let donor_before = token_client.balance(&donor);
    let pool_before = token_client.balance(&pool_addr);

    client.fund_pool(&donor, &100i128);

    let donor_after = token_client.balance(&donor);
    let pool_after = token_client.balance(&pool_addr);

    assert_eq!(donor_before - 100, donor_after);
    assert_eq!(pool_before + 100, pool_after);
}

#[test]
fn test_fund_pool_emits_event() {
    let (env, client, _) = setup();

    let admin = Address::generate(&env);
    let donor = Address::generate(&env);

    let token_id = env.register(TestToken, ());
    let token_client = TestTokenClient::new(&env, &token_id);
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
