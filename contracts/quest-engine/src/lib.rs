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
    ///
    /// # Arguments
    /// * `admin` - The admin address (must match stored admin)
    /// * `status` - The pause status (true = paused, false = unpaused)
    ///
    /// # Panics
    /// * If contract is not initialized
    /// * If admin does not match stored admin
    /// * If admin authentication fails
    pub fn set_pause(env: Env, admin: Address, status: bool) {
        // 1. Fetch 'Admin' address from Instance storage
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");

        // 2. Assert admin == stored_admin
        if admin != stored_admin {
            panic!("Unauthorized");
        }

        // 3. admin.require_auth()
        admin.require_auth();

        // 4. Store pause status in Instance storage
        env.storage().instance().set(&DataKey::IsPaused, &status);
    }

    /// Allows an employer to lock USDC directly in the QuestEngine contract.
    /// This acts as an isolated vault specifically for B2B bounties.
    pub fn create_build_quest(
        env: Env,
        employer: Address,
        reward_amount: i128,
        metadata_hash: BytesN<32>,
    ) -> u32 {
        // 1. employer.require_auth()
        employer.require_auth();

        // 2. Fetch token_client for the USDC asset.
        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);

        // 3. call token_client.transfer(employer, env.current_contract_address(), reward_amount).
        token_client.transfer(&employer, env.current_contract_address(), &reward_amount);

        // 4. Increment Quest ID counter.
        let mut quest_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::QuestCounter)
            .unwrap_or(0);
        quest_id += 1;
        env.storage()
            .instance()
            .set(&DataKey::QuestCounter, &quest_id);

        // 5. Create Quest struct with QuestType::Build.
        let quest = Quest {
            employer: employer.clone(),
            reward_amount,
            quest_type: QuestType::Build,
            metadata_hash,
            active: true,
        };

        // 6. Save to Persistent storage.
        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        // 7. Emit QuestCreated event.
        QuestCreated {
            employer,
            quest_id,
            reward_amount,
        }
        .publish(&env);

        quest_id
    }

    /// Creates an Explore Quest that will be funded by the RewardPool.
    /// Explore Quests are for off-chain actions verified by the admin.
    ///
    /// # Arguments
    /// * `admin` - The admin address (must match stored admin)
    /// * `reward_amount` - The amount to be paid from RewardPool upon verification
    /// * `metadata_hash` - Hash of the quest metadata (description, requirements, etc.)
    ///
    /// # Returns
    /// The ID of the newly created quest
    ///
    /// # Panics
    /// * If admin authentication fails
    /// * If admin does not match stored admin
    /// * If contract is not initialized
    pub fn create_explore_quest(
        env: Env,
        admin: Address,
        reward_amount: i128,
        metadata_hash: BytesN<32>,
    ) -> u32 {
        // 1. admin.require_auth()
        admin.require_auth();

        // 2. Verify admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        assert!(admin == stored_admin, "Unauthorized");

        // 3. Increment Quest ID counter
        let mut quest_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::QuestCounter)
            .unwrap_or(0);
        quest_id += 1;
        env.storage()
            .instance()
            .set(&DataKey::QuestCounter, &quest_id);

        // 4. Create Quest struct with QuestType::Explore
        let quest = Quest {
            employer: admin.clone(),
            reward_amount,
            quest_type: QuestType::Explore,
            metadata_hash,
            active: true,
        };

        // 5. Save to Persistent storage
        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        // 6. Emit QuestCreated event
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
        // 1. learner.require_auth()
        learner.require_auth();

        // 2. Retrieve Quest. Assert it is active and QuestType == Build.
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

        // 3. Construct DataKey::Submission(learner, quest_id).
        let submission_key = DataKey::Submission(learner.clone(), quest_id);

        // 4. Assert a submission doesn't already exist.
        if env.storage().persistent().has(&submission_key) {
            panic!("Submission already exists");
        }

        // 5. Save struct { proof_hash, status: SubmissionStatus::Pending } to storage.
        let submission = Submission {
            proof_hash: proof_hash.clone(),
            status: SubmissionStatus::Pending,
        };
        env.storage().persistent().set(&submission_key, &submission);

        // 6. Emit ProofSubmitted event.
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
        // 0. Check if contract is paused
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        assert!(!is_paused, "Contract is paused");

        // 1. employer.require_auth()
        employer.require_auth();

        // 2. Retrieve Quest. Assert quest.employer == employer.
        let quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");
        if quest.employer != employer {
            panic!("Only the quest employer can review submissions");
        }

        // 3. Retrieve Submission. Assert status == Pending.
        let submission_key = DataKey::Submission(learner.clone(), quest_id);
        let mut submission: Submission = env
            .storage()
            .persistent()
            .get(&submission_key)
            .expect("Submission not found");
        if submission.status != SubmissionStatus::Pending {
            panic!("Submission is not pending review");
        }

        // 4. If approve == true:
        if approve {
            // a. Fetch token_client.transfer(env.current_contract_address(), learner, quest.reward_amount).
            let token_address: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Not initialized");
            let token_client = token::Client::new(&env, &token_address);

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

            // Apply multiplier (basis points: 100 = 1.0x, 120 = 1.2x, etc.)
            // Note: The boosted amount is calculated but capped to base_learner_amount
            // since the quest only has base_learner_amount available after fees.
            // In production, employers should fund quests accounting for potential multipliers,
            // or the boost should come from a separate reward pool contract with proper authorization.
            let calculated_boost = (base_learner_amount * multiplier as i128) / 100;
            let learner_amount = if calculated_boost > base_learner_amount {
                base_learner_amount // Cap to available funds
            } else {
                calculated_boost
            };

            let reward_pool: Address = env
                .storage()
                .instance()
                .get(&DataKey::RewardPool)
                .expect("Not initialized");

            token_client.transfer(&env.current_contract_address(), &reward_pool, &fee);
            token_client.transfer(&env.current_contract_address(), &learner, &learner_amount);

            submission.status = SubmissionStatus::Approved;
        } else {
            // 5. If approve == false:
            // a. Update submission status to Rejected.
            submission.status = SubmissionStatus::Rejected;
        }

        // 6. Save updated submission to Persistent storage.
        env.storage().persistent().set(&submission_key, &submission);

        // 7. Emit SubmissionReviewed event.
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

        quest.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Quest(quest_id), &quest);

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(
            &env.current_contract_address(),
            &employer,
            &quest.reward_amount,
        );

        QuestRefunded {
            employer,
            quest_id,
            amount: quest.reward_amount,
        }
        .publish(&env);
    }

    /// Approves multiple learner submissions in a single transaction.
    /// Executes the full fee-adjusted payout for each learner.
    pub fn batch_review_submissions(
        env: Env,
        employer: Address,
        quest_id: u32,
        learners: Vec<Address>,
    ) {
        // 0. Check if contract is paused
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

            let fee = (quest.reward_amount * 15) / 100;
            let learner_amount = quest.reward_amount - fee;

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
    /// Only the admin can call this function to reward off-chain actions.
    ///
    /// # Arguments
    /// * `admin` - The admin address (must match stored admin)
    /// * `learner` - The learner address to receive the reward
    /// * `quest_id` - The ID of the Explore Quest to verify
    ///
    /// # Panics
    /// * If admin authentication fails
    /// * If admin does not match stored admin
    /// * If quest is not found
    /// * If quest type is not Explore
    /// * If learner has already been verified for this quest
    /// * If contract is not initialized
    pub fn verify_explore_quest(env: Env, admin: Address, learner: Address, quest_id: u32) {
        // 1. admin.require_auth()
        admin.require_auth();

        // 2. Verify admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        assert!(admin == stored_admin, "Unauthorized");

        // 3. Get quest
        let quest: Quest = env
            .storage()
            .persistent()
            .get(&DataKey::Quest(quest_id))
            .expect("Quest not found");

        // 4. Assert quest type is Explore
        assert!(
            quest.quest_type == QuestType::Explore,
            "Not an Explore quest"
        );

        // 5.  NEW: Check if learner was already verified for this quest
        let verification_key = DataKey::ExploreVerification(learner.clone(), quest_id);
        if env.storage().persistent().has(&verification_key) {
            panic!("Learner already verified for this quest");
        }

        // 6. Get reward pool address and create client
        let reward_pool_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardPool)
            .expect("Not initialized");
        let reward_pool_client = RewardPoolClient::new(&env, &reward_pool_address);

        // 7. Distribute reward from RewardPool
        reward_pool_client.distribute_reward(
            &env.current_contract_address(),
            &learner,
            &quest.reward_amount,
        );

        // 8.   NEW: Persist the verification marker after successful payout
        env.storage().persistent().set(&verification_key, &true);

        // 9. Emit ExploreQuestVerified event
        ExploreQuestVerified {
            admin,
            learner,
            quest_id,
            amount: quest.reward_amount,
        }
        .publish(&env);
    }

    pub fn is_explore_verified(env: Env, learner: Address, quest_id: u32) -> bool {
        let key = DataKey::ExploreVerification(learner, quest_id);
        env.storage().persistent().get(&key).unwrap_or(false)
    }
}

#[cfg(test)]
mod test;
