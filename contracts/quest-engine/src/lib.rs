#![no_std]

pub mod types;
use types::{DataKey, Quest, QuestType, Submission, SubmissionStatus};

use soroban_sdk::{
    contract, contractclient, contractevent, contractimpl, token, Address, BytesN, Env, Vec,
};

#[contractclient(name = "StakeVaultClient")]
pub trait StakeVaultInterface {
    fn get_multiplier(env: Env, learner: Address) -> u32;
}

#[contractclient(name = "RewardPoolClient")]
pub trait RewardPoolInterface {
    fn distribute_reward(env: Env, caller: Address, learner: Address, amount: i128);
}

#[contractevent]
pub struct QuestCreated {
    #[topic]
    pub employer: Address,
    #[topic]
    pub quest_id: u32,
    pub reward_amount: i128,
}

#[contractevent]
pub struct ProofSubmitted {
    #[topic]
    pub learner: Address,
    #[topic]
    pub quest_id: u32,
    pub proof_hash: BytesN<32>,
}

#[contractevent]
pub struct SubmissionReviewed {
    #[topic]
    pub employer: Address,
    #[topic]
    pub learner: Address,
    #[topic]
    pub quest_id: u32,
    pub approved: bool,
}

#[contractevent]
pub struct QuestRefunded {
    #[topic]
    pub employer: Address,
    #[topic]
    pub quest_id: u32,
    pub amount: i128,
}

#[contractevent]
pub struct BatchReviewed {
    #[topic]
    pub employer: Address,
    #[topic]
    pub quest_id: u32,
    pub approved_count: u32,
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
}

#[contractevent]
pub struct ExploreQuestVerified {
    #[topic]
    pub admin: Address,
    #[topic]
    pub learner: Address,
    #[topic]
    pub quest_id: u32,
    pub amount: i128,
}

#[contract]
pub struct QuestEngineContract;

#[contractimpl]
impl QuestEngineContract {
    /// Initializes the QuestEngine contract with the token address and admin.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        reward_pool: Address,
        stake_vault: Address,
    ) {
        if env.storage().instance().has(&DataKey::Token) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage()
            .instance()
            .set(&DataKey::RewardPool, &reward_pool);
        env.storage()
            .instance()
            .set(&DataKey::StakeVault, &stake_vault);
        env.storage().instance().set(&DataKey::QuestCounter, &0u32);
    }

    /// Toggles the pause state of the contract (emergency circuit breaker).
    pub fn set_pause(env: Env, admin: Address, status: bool) {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");

        if admin != stored_admin {
            panic!("Unauthorized");
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::IsPaused, &status);
    }

    /// Allows an employer to lock USDC directly in the QuestEngine contract.
    pub fn create_build_quest(
        env: Env,
        employer: Address,
        reward_amount: i128,
        metadata_hash: BytesN<32>,
    ) -> u32 {
        employer.require_auth();

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);

        token_client.transfer(&employer, env.current_contract_address(), &reward_amount);

        let mut quest_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::QuestCounter)
            .unwrap_or(0);
        quest_id += 1;
        env.storage()
            .instance()
            .set(&DataKey::QuestCounter, &quest_id);

        let quest = Quest {
            employer: employer.clone(),
            reward_amount,
            quest_type: QuestType::Build,
            metadata_hash,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        QuestCreated {
            employer,
            quest_id,
            reward_amount,
        }
        .publish(&env);

        quest_id
    }

    /// Creates an Explore Quest that will be funded by the RewardPool.
    pub fn create_explore_quest(
        env: Env,
        admin: Address,
        reward_amount: i128,
        metadata_hash: BytesN<32>,
    ) -> u32 {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        assert!(admin == stored_admin, "Unauthorized");

        let mut quest_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::QuestCounter)
            .unwrap_or(0);
        quest_id += 1;
        env.storage()
            .instance()
            .set(&DataKey::QuestCounter, &quest_id);

        let quest = Quest {
            employer: admin.clone(),
            reward_amount,
            quest_type: QuestType::Explore,
            metadata_hash,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        QuestCreated {
            employer: admin,
            quest_id,
            reward_amount,
        }
        .publish(&env);

        quest_id
    }

    /// Returns a quest by its ID.
    pub fn get_quest(env: Env, quest_id: u32) -> Option<Quest> {
        env.storage().persistent().get(&DataKey::Quest(quest_id))
    }

    /// Allows a learner to submit proof for a build quest.
    pub fn submit_proof(env: Env, learner: Address, quest_id: u32, proof_hash: BytesN<32>) {
        learner.require_auth();

        let quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");
        if !quest.active {
            panic!("Quest is not active");
        }
        if quest.quest_type != QuestType::Build {
            panic!("Only Build quests accept submissions");
        }

        let submission_key = DataKey::Submission(learner.clone(), quest_id);

        if env.storage().persistent().has(&submission_key) {
            panic!("Submission already exists");
        }

        let submission = Submission {
            proof_hash: proof_hash.clone(),
            status: SubmissionStatus::Pending,
        };
        env.storage().persistent().set(&submission_key, &submission);

        ProofSubmitted {
            learner,
            quest_id,
            proof_hash,
        }
        .publish(&env);
    }

    /// Returns a submission by learner and quest ID.
    pub fn get_submission(env: Env, learner: Address, quest_id: u32) -> Option<Submission> {
        env.storage()
            .persistent()
            .get(&DataKey::Submission(learner, quest_id))
    }

    /// Allows an employer to review and approve/reject a learner's submission.
    pub fn review_submission(
        env: Env,
        employer: Address,
        learner: Address,
        quest_id: u32,
        approve: bool,
    ) {
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        assert!(!is_paused, "Contract is paused");

        employer.require_auth();

        let quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");
        if quest.employer != employer {
            panic!("Only the quest employer can review submissions");
        }

        let submission_key = DataKey::Submission(learner.clone(), quest_id);
        let mut submission: Submission = env
            .storage()
            .persistent()
            .get(&submission_key)
            .expect("Submission not found");
        if submission.status != SubmissionStatus::Pending {
            panic!("Submission is not pending review");
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);

        let reward_pool: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardPool)
            .expect("Not initialized");

        if approve {
            let fee = (quest.reward_amount * 15) / 100;
            let base_learner_amount = quest.reward_amount - fee;

            // Fetch stake vault and get multiplier
            let stake_vault_address: Address = env
                .storage()
                .instance()
                .get(&DataKey::StakeVault)
                .expect("Not initialized");
            let stake_vault_client = StakeVaultClient::new(&env, &stake_vault_address);
            let multiplier = stake_vault_client.get_multiplier(&learner);

            // Calculate amount based on multiplier (basis points)
            let final_learner_amount = (base_learner_amount * multiplier as i128) / 100;

            // Transfer fee to reward pool
            token_client.transfer(&env.current_contract_address(), &reward_pool, &fee);

            if multiplier >= 100 {
                // For multipliers >= 100, pay base from escrow
                token_client.transfer(
                    &env.current_contract_address(),
                    &learner,
                    &base_learner_amount,
                );

                // If boosted > base, get the difference from reward pool
                if final_learner_amount > base_learner_amount {
                    let boost_delta = final_learner_amount - base_learner_amount;
                    let reward_pool_client = RewardPoolClient::new(&env, &reward_pool);
                    reward_pool_client.distribute_reward(
                        &env.current_contract_address(),
                        &learner,
                        &boost_delta,
                    );
                }
            } else {
                // For multipliers < 100 (penalty), pay reduced amount from escrow
                // Penalty goes to reward pool
                let penalty = base_learner_amount - final_learner_amount;
                token_client.transfer(
                    &env.current_contract_address(),
                    &learner,
                    &final_learner_amount,
                );
                token_client.transfer(&env.current_contract_address(), &reward_pool, &penalty);
            }

            submission.status = SubmissionStatus::Approved;
        } else {
            submission.status = SubmissionStatus::Rejected;
        }

        env.storage().persistent().set(&submission_key, &submission);

        SubmissionReviewed {
            employer,
            learner,
            quest_id,
            approved: approve,
        }
        .publish(&env);
    }

    pub fn refund_quest(env: Env, employer: Address, quest_id: u32) {
        employer.require_auth();

        let mut quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");

        if quest.employer != employer {
            panic!("Unauthorized");
        }
        if !quest.active {
            panic!("Quest already inactive");
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);

        // Check if there's any balance to refund
        let contract_balance = token_client.balance(&env.current_contract_address());
        if contract_balance == 0 {
            panic!("No unspent balance to refund");
        }

        // Only refund the actual remaining balance
        let refund_amount = contract_balance;

        quest.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        token_client.transfer(&env.current_contract_address(), &employer, &refund_amount);

        QuestRefunded {
            employer,
            quest_id,
            amount: refund_amount,
        }
        .publish(&env);
    }

    /// Approves multiple learner submissions in a single transaction.
    pub fn batch_review_submissions(
        env: Env,
        employer: Address,
        quest_id: u32,
        learners: Vec<Address>,
    ) {
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        assert!(!is_paused, "Contract is paused");

        employer.require_auth();

        let mut quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");
        if quest.employer != employer {
            panic!("Only the quest employer can review submissions");
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);

        let reward_pool: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardPool)
            .expect("Not initialized");

        let fee = (quest.reward_amount * 15) / 100;
        let learner_amount = quest.reward_amount - fee;
        let total_payout = learner_amount * (learners.len() as i128);
        let total_fee = fee * (learners.len() as i128);

        // Check if contract has enough balance
        let contract_balance = token_client.balance(&env.current_contract_address());
        if contract_balance < total_payout + total_fee {
            panic!("Insufficient quest budget");
        }

        let mut approved_count: u32 = 0;
        for learner in learners.iter() {
            let submission_key = DataKey::Submission(learner.clone(), quest_id);
            let mut submission: Submission = env
                .storage()
                .persistent()
                .get(&submission_key)
                .expect("Submission not found");

            if submission.status != SubmissionStatus::Pending {
                panic!("Submission is not pending review");
            }

            token_client.transfer(&env.current_contract_address(), &reward_pool, &fee);
            token_client.transfer(&env.current_contract_address(), &learner, &learner_amount);

            submission.status = SubmissionStatus::Approved;
            env.storage().persistent().set(&submission_key, &submission);

            SubmissionReviewed {
                employer: employer.clone(),
                learner,
                quest_id,
                approved: true,
            }
            .publish(&env);

            approved_count += 1;
        }

        // Mark quest inactive after all approvals
        quest.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        BatchReviewed {
            employer,
            quest_id,
            approved_count,
        }
        .publish(&env);
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

    /// Verifies an Explore Quest completion and triggers payout from RewardPool.
    pub fn verify_explore_quest(env: Env, admin: Address, learner: Address, quest_id: u32) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        assert!(admin == stored_admin, "Unauthorized");

        let quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");

        assert!(
            quest.quest_type == QuestType::Explore,
            "Not an Explore quest"
        );

        // Check if learner already verified this quest
        let verified_key = DataKey::Verified(learner.clone(), quest_id);
        if env.storage().persistent().has(&verified_key) {
            panic!("Learner already verified for this quest");
        }

        // Mark learner as verified
        env.storage().persistent().set(&verified_key, &true);

        let reward_pool_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardPool)
            .expect("Not initialized");
        let reward_pool_client = RewardPoolClient::new(&env, &reward_pool_address);

        reward_pool_client.distribute_reward(
            &env.current_contract_address(),
            &learner,
            &quest.reward_amount,
        );

        ExploreQuestVerified {
            admin,
            learner,
            quest_id,
            amount: quest.reward_amount,
        }
        .publish(&env);
    }
}

#[cfg(test)]
mod test;
