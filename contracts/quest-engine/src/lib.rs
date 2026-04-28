#![no_std]

pub mod types;
use types::{DataKey, Quest, QuestType, Submission, SubmissionStatus};

use soroban_sdk::{contract, contractevent, contractimpl, token, Address, BytesN, Env};

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

#[contract]
pub struct QuestEngineContract;

#[contractimpl]
impl QuestEngineContract {
    /// Initializes the QuestEngine contract with the token address.
    pub fn initialize(env: Env, token: Address) {
        if env.storage().instance().has(&DataKey::Token) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::QuestCounter, &0u32);
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
            token_client.transfer(
                &env.current_contract_address(),
                &learner,
                &quest.reward_amount,
            );

            // b. Update submission status to Approved.
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
}

mod test;
