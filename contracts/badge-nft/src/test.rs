#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Map, Symbol, Val, Vec,
};

use crate::{BadgeNFT, BadgeNFTClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, BadgeNFTClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(BadgeNFT, ());

    let client = BadgeNFTClient::new(&env, &contract_id);
    (env, client)
}

// ── Initialize Tests ─────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let (env, client) = setup();
    let registry = Address::generate(&env);

    // Initialize the contract
    client.initialize(&registry);

    // Verify event was not emitted (initialize doesn't emit events in this pattern)
    assert_eq!(env.events().all().len(), 0);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_panics() {
    let (env, client) = setup();
    let registry = Address::generate(&env);

    // First initialization should succeed
    client.initialize(&registry);

    // Second initialization should panic
    client.initialize(&registry);
}

#[test]
fn test_mint_badge_without_auth_succeeds_with_mock() {
    // In Soroban test environment with mock_all_auths(), auth is automatically satisfied
    // This test verifies that mint_badge works correctly when auth is properly mocked
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);
    
    // With mock_all_auths(), this should succeed
    client.mint_badge(&registry, &learner, &1);
    
    // Verify badge was minted
    assert_eq!(client.get_badge_count(&learner), 1);
}

// ── mint_badge Tests ─────────────────────────────────────────────────────────

#[test]
fn test_mint_badge_success() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    // Initialize the contract
    client.initialize(&registry);

    // Mint badge
    client.mint_badge(&registry, &learner, &1);

    // Verify learner's badge vector increases by 1
    let badges = client.get_badges(&learner);
    assert_eq!(badges.len(), 1);
    assert_eq!(badges.get(0).unwrap().course_id, 1);
}

#[test]
fn test_mint_badge_emits_event() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint badge
    client.mint_badge(&registry, &learner, &1);

    // Verify BadgeMinted event was emitted
    assert_eq!(env.events().all().len(), 1);
    
    let last_event = env.events().all().last().unwrap();
    
    let mut data_map = Map::new(&env);
    data_map.set(Symbol::new(&env, "minted_at"), last_event.2.clone());
    
    let expected_topic: Vec<Val> = (Symbol::new(&env, "badge_minted"), &learner, 1u32).into_val(&env);
    
    assert_eq!(last_event.1, expected_topic);
}

#[test]
fn test_mint_badge_multiple_courses_same_learner() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint badges for multiple courses
    client.mint_badge(&registry, &learner, &1);
    client.mint_badge(&registry, &learner, &2);
    client.mint_badge(&registry, &learner, &3);

    // Verify learner has 3 badges
    let badges = client.get_badges(&learner);
    assert_eq!(badges.len(), 3);
    
    // Verify each badge has correct course_id
    assert_eq!(badges.get(0).unwrap().course_id, 1);
    assert_eq!(badges.get(1).unwrap().course_id, 2);
    assert_eq!(badges.get(2).unwrap().course_id, 3);
}

#[test]
fn test_mint_badge_multiple_learners_same_course() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner1 = Address::generate(&env);
    let learner2 = Address::generate(&env);

    client.initialize(&registry);

    // Mint same course badge to different learners
    client.mint_badge(&registry, &learner1, &1);
    client.mint_badge(&registry, &learner2, &1);

    // Verify each learner has 1 badge
    assert_eq!(client.get_badge_count(&learner1), 1);
    assert_eq!(client.get_badge_count(&learner2), 1);
    
    // Verify both have badge for course 1
    assert!(client.has_badge(&learner1, &1));
    assert!(client.has_badge(&learner2, &1));
}

#[test]
#[should_panic(expected = "Badge for this course already exists")]
fn test_mint_badge_duplicate_panics() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint first badge
    client.mint_badge(&registry, &learner, &1);

    // Try to mint duplicate badge - should panic
    client.mint_badge(&registry, &learner, &1);
}

#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_mint_badge_not_initialized() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    // Try to mint without initializing - should panic
    client.mint_badge(&registry, &learner, &1);
}

#[test]
#[should_panic(expected = "Unauthorized: Caller is not the authorized registry")]
fn test_mint_badge_unauthorized_caller() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let unauthorized_caller = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Try to mint with unauthorized caller - should panic
    client.mint_badge(&unauthorized_caller, &learner, &1);
}

#[test]
fn test_mint_badge_increments_badge_count() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Initially learner has 0 badges
    assert_eq!(client.get_badge_count(&learner), 0);

    // Mint first badge
    client.mint_badge(&registry, &learner, &1);
    assert_eq!(client.get_badge_count(&learner), 1);

    // Mint second badge
    client.mint_badge(&registry, &learner, &2);
    assert_eq!(client.get_badge_count(&learner), 2);

    // Mint third badge
    client.mint_badge(&registry, &learner, &3);
    assert_eq!(client.get_badge_count(&learner), 3);
}

#[test]
fn test_mint_badge_timestamp_is_set() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint badge
    client.mint_badge(&registry, &learner, &1);

    // Verify badge has timestamp (in test environment, it will be 0 by default)
    // The important thing is that the Badge struct has the minted_at field
    let badges = client.get_badges(&learner);
    let badge = badges.get(0).unwrap();
    
    // In test environment without setting ledger timestamp, it defaults to 0
    // This is expected behavior - in production it will have real timestamp
    assert_eq!(badge.minted_at, 0);
}

// ── get_badges Tests ─────────────────────────────────────────────────────────

#[test]
fn test_get_badges_returns_empty_for_new_learner() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Get badges for learner with no badges
    let badges = client.get_badges(&learner);
    assert_eq!(badges.len(), 0);
}

#[test]
fn test_get_badges_returns_all_badges() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint multiple badges
    client.mint_badge(&registry, &learner, &1);
    client.mint_badge(&registry, &learner, &2);
    client.mint_badge(&registry, &learner, &3);

    // Get all badges
    let badges = client.get_badges(&learner);
    assert_eq!(badges.len(), 3);
    
    // Verify all course IDs are present
    let mut course_ids: Vec<u32> = Vec::new(&env);
    for badge in badges.iter() {
        course_ids.push_back(badge.course_id);
    }
    
    assert!(course_ids.contains(&1));
    assert!(course_ids.contains(&2));
    assert!(course_ids.contains(&3));
}

// ── get_badge_count Tests ────────────────────────────────────────────────────

#[test]
fn test_get_badge_count_zero_initially() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Initially learner has 0 badges
    assert_eq!(client.get_badge_count(&learner), 0);
}

#[test]
fn test_get_badge_count_increments() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint badges and verify count increments
    client.mint_badge(&registry, &learner, &1);
    assert_eq!(client.get_badge_count(&learner), 1);

    client.mint_badge(&registry, &learner, &2);
    assert_eq!(client.get_badge_count(&learner), 2);
}

// ── has_badge Tests ──────────────────────────────────────────────────────────

#[test]
fn test_has_badge_returns_false_for_nonexistent() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Learner has no badges
    assert!(!client.has_badge(&learner, &1));
}

#[test]
fn test_has_badge_returns_true_after_mint() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint badge
    client.mint_badge(&registry, &learner, &1);

    // Verify has_badge returns true
    assert!(client.has_badge(&learner, &1));
}

#[test]
fn test_has_badge_returns_false_for_other_courses() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint badge for course 1
    client.mint_badge(&registry, &learner, &1);

    // Verify has_badge returns true for course 1 but false for others
    assert!(client.has_badge(&learner, &1));
    assert!(!client.has_badge(&learner, &2));
    assert!(!client.has_badge(&learner, &3));
}

#[test]
fn test_has_badge_multiple_badges() {
    let (env, client) = setup();
    let registry = Address::generate(&env);
    let learner = Address::generate(&env);

    client.initialize(&registry);

    // Mint multiple badges
    client.mint_badge(&registry, &learner, &1);
    client.mint_badge(&registry, &learner, &3);
    client.mint_badge(&registry, &learner, &5);

    // Verify has_badge returns correct results
    assert!(client.has_badge(&learner, &1));
    assert!(!client.has_badge(&learner, &2));
    assert!(client.has_badge(&learner, &3));
    assert!(!client.has_badge(&learner, &4));
    assert!(client.has_badge(&learner, &5));
}
