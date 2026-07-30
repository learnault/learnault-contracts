#![no_std]
use soroban_sdk::{
    contract, contractclient, contractevent, contractimpl, contracttype, symbol_short, Address,
    BytesN, Env, Symbol, Vec,
};

pub mod types;

pub use types::{DataKey, Proposal};

const BADGE_NFT_KEY: Symbol = symbol_short!("badge");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Badge {
    pub course_id: u32,
    pub minted_at: u64,
}

#[contractclient(name = "BadgeNFTClient")]
pub trait BadgeNFTInterface {
    fn get_badges(env: Env, learner: Address) -> Vec<Badge>;
}

#[contract]
pub struct Governance;

#[contractevent]
pub struct ProposalCreated {
    #[topic]
    pub proposal_id: u32,
    #[topic]
    pub proposer: Address,
}

#[contractevent]
pub struct ProposalExecuted {
    #[topic]
    pub proposal_id: u32,
    pub proposer: Address,
}

#[contractevent]
pub struct ProposalCancelled {
    #[topic]
    pub proposal_id: u32,
    pub cancelled_by: Address,
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
}

#[contractimpl]
impl Governance {
    /// Initializes the governance contract with the admin and BadgeNFT contract address.
    /// Must be called once upon deployment.
    pub fn initialize(env: Env, admin: Address, badge_contract_address: Address) {
        if env.storage().instance().has(&BADGE_NFT_KEY) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&BADGE_NFT_KEY, &badge_contract_address);
    }

    /// Creates a new governance proposal on-chain.
    pub fn create_proposal(
        env: Env,
        proposer: Address,
        metadata_hash: BytesN<32>,
        end_time: u64,
    ) -> u32 {
        proposer.require_auth();

        let current_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ProposalCount)
            .unwrap_or(0);
        let proposal_id = current_count + 1;
        env.storage()
            .instance()
            .set(&DataKey::ProposalCount, &proposal_id);

        let proposal = Proposal {
            id: proposal_id,
            proposer: proposer.clone(),
            metadata_hash,
            votes_for: 0,
            votes_against: 0,
            end_time,
            executed: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        ProposalCreated {
            proposal_id,
            proposer,
        }
        .publish(&env);

        proposal_id
    }

    /// Returns the proposal stored for the given proposal ID.
    pub fn get_proposal(env: Env, proposal_id: u32) -> Proposal {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found")
    }

    /// Casts a vote on a proposal, weighted by the number of badges the voter owns.
    pub fn cast_vote(env: Env, voter: Address, proposal_id: u32, support: bool) {
        voter.require_auth();

        let vote_key = DataKey::UserVote(voter.clone(), proposal_id);
        assert!(!env.storage().persistent().has(&vote_key), "Already voted");

        let badge_contract_address: Address = env
            .storage()
            .instance()
            .get(&BADGE_NFT_KEY)
            .expect("Contract not initialized");
        let badge_client = BadgeNFTClient::new(&env, &badge_contract_address);
        let weight = badge_client.get_badges(&voter).len();

        let mut proposal = Self::get_proposal(env.clone(), proposal_id);
        if support {
            proposal.votes_for = proposal
                .votes_for
                .checked_add(weight)
                .expect("Vote overflow");
        } else {
            proposal.votes_against = proposal
                .votes_against
                .checked_add(weight)
                .expect("Vote overflow");
        }

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage().persistent().set(&vote_key, &true);
    }

    /// Upgrades the contract WASM. Only callable by the Protocol Admin.
    pub fn upgrade_contract(env: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        assert!(admin == stored_admin, "Unauthorized");

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());

        ContractUpgraded {
            admin,
            new_wasm_hash,
        }
        .publish(&env);
    }

    /// Cancels an active proposal. Only callable by the proposer or the Protocol Admin.
    pub fn cancel_proposal(env: Env, caller: Address, proposal_id: u32) {
        caller.require_auth();

        let mut proposal = Self::get_proposal(env.clone(), proposal_id);

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");

        assert!(
            caller == proposal.proposer || caller == stored_admin,
            "Unauthorized"
        );
        assert!(env.ledger().timestamp() < proposal.end_time, "Voting ended");
        assert!(!proposal.executed, "Already executed");

        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        ProposalCancelled {
            proposal_id,
            cancelled_by: caller,
        }
        .publish(&env);
    }

    /// Executes a proposal if it has passed and the voting period has ended.
    /// Marks the proposal as executed so the admin knows to action the approved change.
    pub fn execute_proposal(env: Env, proposal_id: u32) {
        let mut proposal = Self::get_proposal(env.clone(), proposal_id);

        assert!(
            env.ledger().timestamp() > proposal.end_time,
            "Voting still active"
        );
        assert!(
            proposal.votes_for > proposal.votes_against,
            "Proposal rejected"
        );
        assert!(!proposal.executed, "Already executed");

        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        ProposalExecuted {
            proposal_id,
            proposer: proposal.proposer,
        }
        .publish(&env);
    }
}

#[cfg(test)]
mod test;
