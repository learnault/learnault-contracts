#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env,
};

use crate::{QuestEngineContract, QuestEngineContractClient};
use crate::types::QuestType;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, QuestEngineContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    // Create a SAC token for USDC
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();

    // Initialize the contract with token
    client.initialize(&token_id);

    (env, client, token_id)
}

fn mint_tokens(env: &Env, token_id: &Address, to: &Address, amount: &i128) {
    let sac_client = soroban_sdk::token::StellarAssetClient::new(env, token_id);
    sac_client.mint(to, amount);
}

fn token_balance(env: &Env, token_id: &Address, of: &Address) -> i128 {
    soroban_sdk::token::Client::new(env, token_id).balance(of)
}

// ── Initialize Tests ─────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_panics() {
    let (_env, client, token_id) = setup();
    // setup() already initializes once, this second call should panic
    client.initialize(&token_id);
}

// ── create_build_quest Tests ─────────────────────────────────────────────────

#[test]
fn test_create_build_quest_success() {
    let (env, client, token_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 1_000;
    let metadata_hash = BytesN::from_array(&env, &[1u8; 32]);

    // Fund the employer
    mint_tokens(&env, &token_id, &employer, &reward_amount);
    assert_eq!(token_balance(&env, &token_id, &employer), reward_amount);

    // Create a build quest
    let quest_id = client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    // Quest ID should be 1 (first quest)
    assert_eq!(quest_id, 1);

    // ✅ Acceptance: QuestEngine contract balance increases
    assert_eq!(token_balance(&env, &token_id, &client.address), reward_amount);
    assert_eq!(token_balance(&env, &token_id, &employer), 0);

    // ✅ Acceptance: Quest is saved as a Build type
    let quest = client.get_quest(&quest_id).unwrap();
    assert_eq!(quest.employer, employer);
    assert_eq!(quest.reward_amount, reward_amount);
    assert_eq!(quest.quest_type, QuestType::Build);
    assert_eq!(quest.metadata_hash, metadata_hash);
    assert!(quest.active);
}

#[test]
fn test_create_build_quest_emits_event() {
    let (env, client, token_id) = setup();
    let employer = Address::generate(&env);
    let reward_amount: i128 = 500;
    let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);

    mint_tokens(&env, &token_id, &employer, &reward_amount);

    client.create_build_quest(&employer, &reward_amount, &metadata_hash);

    // Verify QuestCreated event was emitted
    let events = env.events().all();
    assert!(events.len() >= 1, "Expected at least 1 event, got {}", events.len());
}

#[test]
fn test_create_build_quest_increments_ids() {
    let (env, client, token_id) = setup();
    let employer = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[3u8; 32]);

    // Fund enough for 3 quests
    mint_tokens(&env, &token_id, &employer, &3000);

    let id1 = client.create_build_quest(&employer, &1000, &metadata_hash);
    let id2 = client.create_build_quest(&employer, &1000, &metadata_hash);
    let id3 = client.create_build_quest(&employer, &1000, &metadata_hash);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);

    // Verify all quests exist and are Build type
    for id in [id1, id2, id3] {
        let quest = client.get_quest(&id).unwrap();
        assert_eq!(quest.quest_type, QuestType::Build);
        assert!(quest.active);
    }

    // Total contract balance should be 3000
    assert_eq!(token_balance(&env, &token_id, &client.address), 3000);
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_create_quest_without_init_panics() {
    let env = Env::default();
    env.mock_all_auths();

    // Register contract but do NOT initialize
    let contract_id = env.register(QuestEngineContract, ());
    let client = QuestEngineContractClient::new(&env, &contract_id);

    let employer = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.create_build_quest(&employer, &100, &metadata_hash);
}

#[test]
fn test_get_quest_returns_none_for_nonexistent() {
    let (_env, client, _token_id) = setup();
    assert_eq!(client.get_quest(&999), None);
}

#[test]
fn test_create_build_quest_multiple_employers() {
    let (env, client, token_id) = setup();
    let employer1 = Address::generate(&env);
    let employer2 = Address::generate(&env);
    let metadata_hash = BytesN::from_array(&env, &[4u8; 32]);

    mint_tokens(&env, &token_id, &employer1, &500);
    mint_tokens(&env, &token_id, &employer2, &700);

    let id1 = client.create_build_quest(&employer1, &500, &metadata_hash);
    let id2 = client.create_build_quest(&employer2, &700, &metadata_hash);

    let quest1 = client.get_quest(&id1).unwrap();
    let quest2 = client.get_quest(&id2).unwrap();

    assert_eq!(quest1.employer, employer1);
    assert_eq!(quest1.reward_amount, 500);
    assert_eq!(quest2.employer, employer2);
    assert_eq!(quest2.reward_amount, 700);

    // Total contract balance
    assert_eq!(token_balance(&env, &token_id, &client.address), 1200);
}
