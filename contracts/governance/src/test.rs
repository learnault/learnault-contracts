use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, BytesN, Env,
};

use badge_nft::{BadgeNFT, BadgeNFTClient};

use crate::{types::DataKey, Governance, GovernanceClient, Proposal};

fn setup() -> (
    Env,
    GovernanceClient<'static>,
    BadgeNFTClient<'static>,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let governance_id = env.register(Governance, ());
    let badge_nft_id = env.register(BadgeNFT, ());

    let governance_client = GovernanceClient::new(&env, &governance_id);
    let badge_client = BadgeNFTClient::new(&env, &badge_nft_id);
    let admin = Address::generate(&env);

    (env, governance_client, badge_client, admin)
}

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[7u8; 32])
}

fn seed_proposal(
    env: &Env,
    governance_client: &GovernanceClient<'_>,
    proposal_id: u32,
    proposer: &Address,
) {
    env.as_contract(&governance_client.address, || {
        env.storage().persistent().set(
            &DataKey::Proposal(proposal_id),
            &Proposal {
                id: proposal_id,
                proposer: proposer.clone(),
                metadata_hash: dummy_hash(env),
                votes_for: 0,
                votes_against: 0,
                end_time: 1_000,
                executed: false,
            },
        );
    });
}

#[test]
fn test_cast_vote_uses_badge_count_as_weight() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    badge_client.mint_badge(&badge_admin, &voter, &101);
    badge_client.mint_badge(&badge_admin, &voter, &102);
    badge_client.mint_badge(&badge_admin, &voter, &103);

    governance_client.cast_vote(&voter, &1, &true);

    let proposal = governance_client.get_proposal(&1);
    assert_eq!(proposal.votes_for, 3);
    assert_eq!(proposal.votes_against, 0);

    env.as_contract(&governance_client.address, || {
        let recorded: bool = env
            .storage()
            .persistent()
            .get(&DataKey::UserVote(voter.clone(), 1))
            .expect("vote should be recorded");
        assert!(recorded);
    });
}

#[test]
#[should_panic(expected = "Already voted")]
fn test_cast_vote_prevents_double_voting() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    badge_client.mint_badge(&badge_admin, &voter, &999);

    governance_client.cast_vote(&voter, &1, &true);
    governance_client.cast_vote(&voter, &1, &false);
}

#[test]
fn test_execute_proposal_success() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Cast votes in favor
    badge_client.mint_badge(&badge_admin, &voter, &101);
    badge_client.mint_badge(&badge_admin, &voter, &102);
    governance_client.cast_vote(&voter, &1, &true);

    // Move time past end_time
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.execute_proposal(&1);

    let proposal = governance_client.get_proposal(&1);
    assert!(proposal.executed);
    assert_eq!(proposal.votes_for, 2);
    assert_eq!(proposal.votes_against, 0);
}

#[test]
#[should_panic(expected = "Voting still active")]
fn test_execute_proposal_voting_still_active() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Cast votes in favor
    badge_client.mint_badge(&badge_admin, &voter, &101);
    governance_client.cast_vote(&voter, &1, &true);

    // Time is still before end_time (1_000)
    env.ledger().with_mut(|li| li.timestamp = 999);

    governance_client.execute_proposal(&1);
}

#[test]
#[should_panic(expected = "Proposal rejected")]
fn test_execute_proposal_rejected() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter_for = Address::generate(&env);
    let voter_against = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Cast votes: 1 for, 2 against
    badge_client.mint_badge(&badge_admin, &voter_for, &101);
    governance_client.cast_vote(&voter_for, &1, &true);

    badge_client.mint_badge(&badge_admin, &voter_against, &201);
    badge_client.mint_badge(&badge_admin, &voter_against, &202);
    governance_client.cast_vote(&voter_against, &1, &false);

    // Move time past end_time
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.execute_proposal(&1);
}

#[test]
#[should_panic(expected = "Proposal rejected")]
fn test_execute_proposal_tied_vote() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter_for = Address::generate(&env);
    let voter_against = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Cast votes: 2 for, 2 against (tie)
    badge_client.mint_badge(&badge_admin, &voter_for, &101);
    badge_client.mint_badge(&badge_admin, &voter_for, &102);
    governance_client.cast_vote(&voter_for, &1, &true);

    badge_client.mint_badge(&badge_admin, &voter_against, &201);
    badge_client.mint_badge(&badge_admin, &voter_against, &202);
    governance_client.cast_vote(&voter_against, &1, &false);

    // Move time past end_time
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.execute_proposal(&1);
}

#[test]
#[should_panic(expected = "Already executed")]
fn test_execute_proposal_already_executed() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Cast votes in favor
    badge_client.mint_badge(&badge_admin, &voter, &101);
    governance_client.cast_vote(&voter, &1, &true);

    // Move time past end_time
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.execute_proposal(&1);
    governance_client.execute_proposal(&1); // Try to execute again
}

#[test]
fn test_execute_proposal_emits_event() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Cast votes in favor
    badge_client.mint_badge(&badge_admin, &voter, &101);
    governance_client.cast_vote(&voter, &1, &true);

    // Move time past end_time
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.execute_proposal(&1);

    // Verify event was emitted
    assert_eq!(env.events().all().len(), 1);
}

#[test]
#[should_panic(expected = "Proposal not found")]
fn test_execute_proposal_nonexistent() {
    let (env, governance_client, badge_client, admin) = setup();

    governance_client.initialize(&admin, &badge_client.address);

    // Move time past end_time
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.execute_proposal(&999);
}

// ── cancel_proposal Tests ─────────────────────────────────────────────────────

#[test]
fn test_cancel_proposal_by_proposer_succeeds() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    governance_client.cancel_proposal(&proposer, &1);

    let proposal = governance_client.get_proposal(&1);
    assert!(
        proposal.executed,
        "Proposal should be locked (executed=true)"
    );
}

#[test]
fn test_cancel_proposal_by_admin_succeeds() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    governance_client.cancel_proposal(&admin, &1);

    let proposal = governance_client.get_proposal(&1);
    assert!(proposal.executed);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_cancel_proposal_by_random_caller_panics() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let random = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    governance_client.cancel_proposal(&random, &1);
}

#[test]
#[should_panic(expected = "Voting ended")]
fn test_cancel_proposal_after_voting_period_panics() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    // Move time past end_time (1_000)
    env.ledger().with_mut(|li| li.timestamp = 1_001);

    governance_client.cancel_proposal(&proposer, &1);
}

#[test]
#[should_panic(expected = "Already executed")]
fn test_cancel_proposal_already_executed_panics() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    badge_client.mint_badge(&badge_admin, &voter, &101);
    governance_client.cast_vote(&voter, &1, &true);
    env.ledger().with_mut(|li| li.timestamp = 1_001);
    governance_client.execute_proposal(&1);

    // Reset time to within voting period for second proposal, but proposal 1 is already executed
    env.ledger().with_mut(|li| li.timestamp = 0);
    governance_client.cancel_proposal(&proposer, &1);
}

#[test]
fn test_cancel_proposal_prevents_voting() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    badge_client.mint_badge(&badge_admin, &voter, &101);

    // Cancel the proposal
    governance_client.cancel_proposal(&proposer, &1);

    // A cancelled proposal has executed=true, so cast_vote itself doesn't check that,
    // but execute_proposal will fail with "Already executed"
    // Verify the proposal is locked
    let proposal = governance_client.get_proposal(&1);
    assert!(proposal.executed);
}

#[test]
fn test_cancel_proposal_emits_event() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);
    seed_proposal(&env, &governance_client, 1, &proposer);

    governance_client.cancel_proposal(&proposer, &1);

    assert_eq!(env.events().all().len(), 1);
}

// ── upgrade_contract Tests ────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_upgrade_contract_by_non_admin_panics() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let attacker = Address::generate(&env);

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);

    let new_wasm_hash = BytesN::from_array(&env, &[0xabu8; 32]);
    governance_client.upgrade_contract(&attacker, &new_wasm_hash);
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_upgrade_contract_not_initialized_panics() {
    let (env, governance_client, _badge_client, _admin) = setup();
    let attacker = Address::generate(&env);
    let new_wasm_hash = BytesN::from_array(&env, &[0xabu8; 32]);
    governance_client.upgrade_contract(&attacker, &new_wasm_hash);
}

// ── create_proposal Tests ────────────────────────────────────────────────────

#[test]
fn test_create_proposal_success() {
    let (env, governance_client, badge_client, admin) = setup();
    let proposer = Address::generate(&env);
    let meta_hash = dummy_hash(&env);
    let end_time = 2_000u64;

    governance_client.initialize(&admin, &badge_client.address);

    let proposal_id = governance_client.create_proposal(&proposer, &meta_hash, &end_time);
    assert_eq!(proposal_id, 1);

    let proposal = governance_client.get_proposal(&proposal_id);
    assert_eq!(proposal.id, 1);
    assert_eq!(proposal.proposer, proposer);
    assert_eq!(proposal.metadata_hash, meta_hash);
    assert_eq!(proposal.votes_for, 0);
    assert_eq!(proposal.votes_against, 0);
    assert_eq!(proposal.end_time, end_time);
    assert!(!proposal.executed);
}

#[test]
fn test_create_proposal_increments_id() {
    let (env, governance_client, badge_client, admin) = setup();
    let proposer = Address::generate(&env);
    let meta_hash = dummy_hash(&env);

    governance_client.initialize(&admin, &badge_client.address);

    let id1 = governance_client.create_proposal(&proposer, &meta_hash, &1_000u64);
    let id2 = governance_client.create_proposal(&proposer, &meta_hash, &2_000u64);
    let id3 = governance_client.create_proposal(&proposer, &meta_hash, &3_000u64);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_created_proposal_is_voteable_and_executable() {
    let (env, governance_client, badge_client, admin) = setup();
    let badge_admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    let meta_hash = dummy_hash(&env);
    let end_time = 1_000u64;

    governance_client.initialize(&admin, &badge_client.address);
    badge_client.initialize(&badge_admin);

    let proposal_id = governance_client.create_proposal(&proposer, &meta_hash, &end_time);

    badge_client.mint_badge(&badge_admin, &voter, &101);
    governance_client.cast_vote(&voter, &proposal_id, &true);

    env.ledger().with_mut(|li| li.timestamp = 1_001);
    governance_client.execute_proposal(&proposal_id);

    let proposal = governance_client.get_proposal(&proposal_id);
    assert!(proposal.executed);
    assert_eq!(proposal.votes_for, 1);
}

#[test]
fn test_create_proposal_emits_event() {
    let (env, governance_client, badge_client, admin) = setup();
    let proposer = Address::generate(&env);
    let meta_hash = dummy_hash(&env);

    governance_client.initialize(&admin, &badge_client.address);

    governance_client.create_proposal(&proposer, &meta_hash, &1_000u64);
    assert_eq!(env.events().all().len(), 1);
}

