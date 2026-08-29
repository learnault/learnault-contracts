#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, Address, BytesN, Env, String};

pub mod types;
use types::{CompletionPolicy, Course, DataKey};

use badge_nft::BadgeNFTClient;
use reward_pool::RewardPoolClient;

#[contract]
pub struct CourseRegistry;

#[contractevent]
pub struct MetadataUpdated {
    #[topic]
    pub id: u32,
    #[topic]
    pub instructor: Address,
    pub new_hash: BytesN<32>,
}

#[contractevent]
pub struct CourseCreated {
    #[topic]
    pub id: u32,
    #[topic]
    pub instructor: Address,
    pub total_modules: u32,
    pub reward_amount: i128,
    pub completion_policy: CompletionPolicy,
}

#[contractevent]
pub struct CourseStatusChanged {
    #[topic]
    pub id: u32,
    pub active: bool,
}

#[contractevent]
pub struct OwnershipTransferred {
    #[topic]
    pub course_id: u32,
    #[topic]
    pub previous_instructor: Address,
    pub new_instructor: Address,
}

#[contractevent]
pub struct CourseRewardUpdated {
    #[topic]
    pub course_id: u32,
    pub old_reward: i128,
    pub new_reward: i128,
}

#[contractevent]
pub struct ModuleCompleted {
    #[topic]
    pub learner: Address,
    #[topic]
    pub course_id: u32,
    pub new_progress: u32,
}

#[contractevent]
pub struct CourseCompleted {
    #[topic]
    pub learner: Address,
    #[topic]
    pub course_id: u32,
    pub reward_amount: i128,
    pub badge_minted: bool,
    pub reward_paid: bool,
    pub policy: CompletionPolicy,
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
}

#[contractevent]
pub struct CompletionPolicyUpdated {
    #[topic]
    pub course_id: u32,
    pub old_policy: CompletionPolicy,
    pub new_policy: CompletionPolicy,
    pub updated_by: Address,
}

#[contractevent]
pub struct IntegrationAddressUpdated {
    #[topic]
    pub course_id: u32,
    pub integration_type: String,
    pub address: Option<Address>,
    pub updated_by: Address,
}

#[contractimpl]
impl CourseRegistry {
    /// Sets the official Protocol Admin. Must be called once upon deployment.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Registers the RewardPool contract address so the registry can trigger payouts on completion.
    /// Only callable by the Protocol Admin.
    pub fn set_reward_pool_address(env: Env, admin: Address, reward_pool_address: Address) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        env.storage()
            .instance()
            .set(&DataKey::RewardPoolAddress, &reward_pool_address);
    }

    /// Registers the BadgeNFT contract address so the registry can mint badges on completion.
    /// Only callable by the Protocol Admin.
    pub fn set_badge_nft_address(env: Env, admin: Address, badge_nft_address: Address) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        env.storage()
            .instance()
            .set(&DataKey::BadgeNftAddress, &badge_nft_address);
    }

    /// Registers a new course on-chain with completion policy.
    pub fn create_course(
        env: Env,
        admin: Address,
        instructor: Address,
        total_modules: u32,
        metadata_hash: BytesN<32>,
        reward_amount: i128,
        completion_policy: Option<CompletionPolicy>,
    ) -> u32 {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        assert!(total_modules > 0, "total_modules must be greater than 0");
        assert!(reward_amount >= 0, "reward_amount must be non-negative");

        let current_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CourseCount)
            .unwrap_or(0);
        let new_id = current_count + 1;
        env.storage().instance().set(&DataKey::CourseCount, &new_id);

        let policy = completion_policy.unwrap_or_default();
        let course = Course {
            instructor: instructor.clone(),
            total_modules,
            metadata_hash,
            active: true,
            reward_amount,
            completion_policy: policy.clone(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Course(new_id), &course);

        // Store policy separately for easy access
        env.storage()
            .persistent()
            .set(&DataKey::CompletionPolicy(new_id), &policy);

        CourseCreated {
            id: new_id,
            instructor,
            total_modules,
            reward_amount,
            completion_policy: policy,
        }
        .publish(&env);

        new_id
    }

    /// Updates the completion policy for a course. Only callable by the Protocol Admin.
    pub fn set_completion_policy(
        env: Env,
        admin: Address,
        course_id: u32,
        new_policy: CompletionPolicy,
    ) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id))
            .expect("Course not found");

        let old_policy = course.completion_policy.clone();
        course.completion_policy = new_policy.clone();

        env.storage()
            .persistent()
            .set(&DataKey::Course(course_id), &course);
        env.storage()
            .persistent()
            .set(&DataKey::CompletionPolicy(course_id), &new_policy);

        CompletionPolicyUpdated {
            course_id,
            old_policy,
            new_policy,
            updated_by: admin,
        }
        .publish(&env);
    }

    /// Gets the completion policy for a course.
    pub fn get_completion_policy(env: Env, course_id: u32) -> CompletionPolicy {
        env.storage()
            .persistent()
            .get(&DataKey::CompletionPolicy(course_id))
            .unwrap_or_default()
    }

    /// Updates integration addresses for a course. Only callable by the Protocol Admin.
    pub fn set_course_integrations(
        env: Env,
        admin: Address,
        course_id: u32,
        reward_pool: Option<Address>,
        badge_nft: Option<Address>,
    ) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        // Verify course exists
        let _course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id))
            .expect("Course not found");

        if let Some(reward) = reward_pool {
            env.storage()
                .instance()
                .set(&DataKey::RewardPoolAddress, &reward);

            IntegrationAddressUpdated {
                course_id,
                integration_type: String::from_str(&env, "reward_pool"),
                address: Some(reward),
                updated_by: admin.clone(),
            }
            .publish(&env);
        }

        if let Some(badge) = badge_nft {
            env.storage()
                .instance()
                .set(&DataKey::BadgeNftAddress, &badge);

            IntegrationAddressUpdated {
                course_id,
                integration_type: String::from_str(&env, "badge_nft"),
                address: Some(badge),
                updated_by: admin,
            }
            .publish(&env);
        }
    }

    /// Updates the IPFS metadata hash for a course. Only callable by the course instructor.
    pub fn update_metadata(env: Env, id: u32, new_hash: BytesN<32>) {
        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        course.instructor.require_auth();

        let instructor = course.instructor.clone();
        course.metadata_hash = new_hash.clone();

        env.storage()
            .persistent()
            .set(&DataKey::Course(id), &course);

        MetadataUpdated {
            id,
            instructor,
            new_hash,
        }
        .publish(&env);
    }

    /// Enrolls a learner in an active course, initializing their progress to 0.
    pub fn enroll(env: Env, learner: Address, id: u32) {
        learner.require_auth();

        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        assert!(course.active, "Course is not active");

        let progress_key = DataKey::Progress(learner.clone(), id);
        assert!(
            !env.storage().persistent().has(&progress_key),
            "Learner already enrolled"
        );

        env.storage().persistent().set(&progress_key, &0u32);
    }

    /// Helper to check the current total number of courses.
    pub fn course_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CourseCount)
            .unwrap_or(0)
    }

    /// Toggles a course's active status. Only callable by the Protocol Admin.
    pub fn set_course_status(env: Env, admin: Address, id: u32, active: bool) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        course.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Course(id), &course);

        CourseStatusChanged { id, active }.publish(&env);
    }

    /// Updates the completion reward amount for a course. Only callable by the Protocol Admin.
    pub fn update_course_reward(env: Env, admin: Address, id: u32, new_reward_amount: i128) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        assert!(new_reward_amount >= 0, "reward_amount must be non-negative");

        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        let old_reward = course.reward_amount;
        course.reward_amount = new_reward_amount;

        env.storage()
            .persistent()
            .set(&DataKey::Course(id), &course);

        CourseRewardUpdated {
            course_id: id,
            old_reward,
            new_reward: new_reward_amount,
        }
        .publish(&env);
    }

    /// Returns true if the learner has completed all modules in the course.
    pub fn is_course_finished(env: Env, learner: Address, id: u32) -> bool {
        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        let progress: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Progress(learner, id))
            .unwrap_or(0);

        progress >= course.total_modules
    }

    /// Returns the full details of a specific course.
    pub fn get_course(env: Env, id: u32) -> Course {
        env.storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found")
    }

    /// Returns a learner's completed module count for a course. Returns 0 if the learner has not enrolled.
    pub fn get_progress(env: Env, learner: Address, id: u32) -> u32 {
        let key = DataKey::Progress(learner, id);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Transfers ownership of a course to a new instructor address.
    /// Only callable by the current instructor of the course.
    pub fn transfer_ownership(
        env: Env,
        current_instructor: Address,
        new_instructor: Address,
        course_id: u32,
    ) {
        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id))
            .expect("Course not found");

        assert!(
            course.instructor == current_instructor,
            "Unauthorized: Caller is not the course instructor"
        );

        current_instructor.require_auth();

        course.instructor = new_instructor.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Course(course_id), &course);

        OwnershipTransferred {
            course_id,
            previous_instructor: current_instructor,
            new_instructor,
        }
        .publish(&env);
    }

    /// Records a learner's completion of a module after off-chain quiz validation.
    /// Only callable by the authorized verifier (protocol admin).
    pub fn complete_module(env: Env, verifier: Address, learner: Address, id: u32) {
        verifier.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            verifier == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        let current_progress: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Progress(learner.clone(), id))
            .unwrap_or(0);

        assert!(
            current_progress < course.total_modules,
            "Course already completed"
        );

        let new_progress = current_progress + 1;

        env.storage()
            .persistent()
            .set(&DataKey::Progress(learner.clone(), id), &new_progress);

        ModuleCompleted {
            learner: learner.clone(),
            course_id: id,
            new_progress,
        }
        .publish(&env);

        // If the learner just finished the final module, handle completion side effects
        if new_progress == course.total_modules {
            // Enforce completion policy before executing side effects
            Self::enforce_completion_policy(&env, &course);

            // Execute side effects
            let (reward_paid, badge_minted) =
                Self::execute_completion_side_effects(&env, &course, learner.clone(), id);

            CourseCompleted {
                learner: learner.clone(),
                course_id: id,
                reward_amount: course.reward_amount,
                badge_minted,
                reward_paid,
                policy: course.completion_policy.clone(),
            }
            .publish(&env);
        }
    }

    /// Enforces the completion policy by validating required integrations
    fn enforce_completion_policy(env: &Env, course: &Course) {
        match course.completion_policy {
            CompletionPolicy::Optional => {}
            CompletionPolicy::RewardRequired => {
                if env
                    .storage()
                    .instance()
                    .get::<DataKey, Address>(&DataKey::RewardPoolAddress)
                    .is_none()
                {
                    panic!("Completion policy violation: Reward pool required but not configured");
                }
            }
            CompletionPolicy::BadgeRequired => {
                if env
                    .storage()
                    .instance()
                    .get::<DataKey, Address>(&DataKey::BadgeNftAddress)
                    .is_none()
                {
                    panic!("Completion policy violation: Badge NFT required but not configured");
                }
            }
            CompletionPolicy::BothRequired => {
                if env
                    .storage()
                    .instance()
                    .get::<DataKey, Address>(&DataKey::RewardPoolAddress)
                    .is_none()
                {
                    panic!("Completion policy violation: Reward pool required but not configured");
                }
                if env
                    .storage()
                    .instance()
                    .get::<DataKey, Address>(&DataKey::BadgeNftAddress)
                    .is_none()
                {
                    panic!("Completion policy violation: Badge NFT required but not configured");
                }
            }
        }
    }

    /// Executes completion side effects with explicit handling
    fn execute_completion_side_effects(
        env: &Env,
        course: &Course,
        learner: Address,
        course_id: u32,
    ) -> (bool, bool) {
        let mut reward_paid = false;
        let mut badge_minted = false;

        // Only process reward if policy is not Optional OR if reward is specifically configured
        if course.completion_policy != CompletionPolicy::Optional {
            // Handle reward payout if configured
            if let Some(reward_pool_address) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::RewardPoolAddress)
            {
                let reward = course.reward_amount;
                if reward > 0 {
                    let reward_pool = RewardPoolClient::new(env, &reward_pool_address);
                    match reward_pool.try_distribute_reward(
                        &env.current_contract_address(),
                        &learner,
                        &reward,
                    ) {
                        Ok(_) => {
                            reward_paid = true;
                        }
                        Err(_e) => {
                            // If reward is required by policy, fail the completion
                            match course.completion_policy {
                                CompletionPolicy::RewardRequired
                                | CompletionPolicy::BothRequired => {
                                    panic!("Reward payout failed but required by policy");
                                }
                                _ => {
                                    // Silently skip for optional policy
                                    soroban_sdk::log!(
                                        env,
                                        "Warning: Reward payout failed for course {}",
                                        course_id
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Only process badge if policy is not Optional
        if course.completion_policy != CompletionPolicy::Optional {
            // Handle badge minting if configured
            if let Some(badge_nft_address) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::BadgeNftAddress)
            {
                let badge_nft = BadgeNFTClient::new(env, &badge_nft_address);
                match badge_nft.try_mint_badge(
                    &env.current_contract_address(),
                    &learner,
                    &course_id,
                ) {
                    Ok(_) => {
                        badge_minted = true;
                    }
                    Err(_e) => {
                        // If badge is required by policy, fail the completion
                        match course.completion_policy {
                            CompletionPolicy::BadgeRequired | CompletionPolicy::BothRequired => {
                                panic!("Badge minting failed but required by policy");
                            }
                            _ => {
                                // Silently skip for optional policy
                                soroban_sdk::log!(
                                    env,
                                    "Warning: Badge minting failed for course {}",
                                    course_id
                                );
                            }
                        }
                    }
                }
            }
        }

        (reward_paid, badge_minted)
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
}

mod test;
