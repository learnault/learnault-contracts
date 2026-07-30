#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, Address, BytesN, Env};

pub mod types;
use types::{Course, DataKey};

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
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
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

    /// Registers a new course on-chain.
    pub fn create_course(
        env: Env,
        admin: Address,
        instructor: Address,
        total_modules: u32,
        metadata_hash: BytesN<32>,
        reward_amount: i128,
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

        let course = Course {
            instructor: instructor.clone(),
            total_modules,
            metadata_hash,
            active: true,
            reward_amount,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Course(new_id), &course);

        CourseCreated {
            id: new_id,
            instructor,
            total_modules,
            reward_amount,
        }
        .publish(&env);

        new_id
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
        // 1. Authenticate the admin cryptographically
        admin.require_auth();

        // 2. Verify caller is the officially registered Protocol Admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            admin == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        // 3. Retrieve the course using the CORRECT DataKey
        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        // 4. Update the active status and save it
        course.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Course(id), &course);

        // 5. Emit the standard event
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
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `id` - The course ID
    ///
    /// # Returns
    /// The Course struct if found
    ///
    /// # Panics
    /// Panics if the course ID is invalid (course doesn't exist in storage)
    pub fn get_course(env: Env, id: u32) -> Course {
        // 1. Construct DataKey::Course(id)
        let key = DataKey::Course(id);

        // 2. Fetch Course struct from Persistent storage
        // 3. Assert course exists (panic if not found)
        env.storage()
            .persistent()
            .get(&key)
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
        // 1. Authenticate the verifier's signature
        verifier.require_auth();

        // 2. Verify the verifier is the authorized protocol admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            verifier == stored_admin,
            "Unauthorized: Caller is not the protocol admin"
        );

        // 3. Retrieve the course to validate it exists and get total_modules
        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .expect("Course not found");

        // 4. Retrieve current progress (defaults to 0 if not set)
        let current_progress: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Progress(learner.clone(), id))
            .unwrap_or(0);

        // 5. Assert current progress is less than total_modules
        assert!(
            current_progress < course.total_modules,
            "Course already completed"
        );

        // 6. Increment progress by 1
        let new_progress = current_progress + 1;

        // 7. Save new progress to persistent storage
        env.storage()
            .persistent()
            .set(&DataKey::Progress(learner.clone(), id), &new_progress);

        // 8. Emit ModuleCompleted event
        ModuleCompleted {
            learner: learner.clone(),
            course_id: id,
            new_progress,
        }
        .publish(&env);

        // 9. If the learner just finished the final module, mint their soulbound badge
        if new_progress == course.total_modules {
            if let Some(badge_nft_address) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::BadgeNftAddress)
            {
                let badge_nft = BadgeNFTClient::new(&env, &badge_nft_address);
                badge_nft.mint_badge(&env.current_contract_address(), &learner, &id);
            }

            // 10. Trigger reward distribution if RewardPool address is configured
            if let Some(reward_pool_address) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::RewardPoolAddress)
            {
                let reward = course.reward_amount;
                if reward > 0 {
                    let reward_pool = RewardPoolClient::new(&env, &reward_pool_address);
                    reward_pool.distribute_reward(
                        &env.current_contract_address(),
                        &learner,
                        &reward,
                    );
                }

                CourseCompleted {
                    learner: learner.clone(),
                    course_id: id,
                    reward_amount: reward,
                }
                .publish(&env);
            }
        }
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
