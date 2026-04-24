#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, Address, Env, Vec};

pub mod types;
use types::{Badge, DataKey};

#[contract]
pub struct BadgeNFT;

#[contractevent]
pub struct BadgeMinted {
    #[topic]
    pub learner: Address,
    #[topic]
    pub course_id: u32,
    pub minted_at: u64,
}

#[contractimpl]
impl BadgeNFT {
    /// Initializes the BadgeNFT contract with the authorized registry address.
    /// Must be called once upon deployment.
    ///
    /// # Arguments
    /// * `admin` - The CourseRegistry contract address authorized to mint badges
    ///
    /// # Panics
    /// * If contract is already initialized
    pub fn initialize(env: Env, admin: Address) {
        // 1. Check if already initialized
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        // 2. Store admin (registry) in Instance storage
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Mints a Soulbound Token (badge) directly to the learner's address.
    /// Only the official protocol registry can trigger this.
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be the authorized registry)
    /// * `learner` - The learner address to receive the badge
    /// * `course_id` - The course ID for which the badge is being minted
    ///
    /// # Panics
    /// * If caller authentication fails
    /// * If caller is not the authorized registry
    /// * If learner already has a badge for this course_id (duplicate minting)
    pub fn mint_badge(env: Env, caller: Address, learner: Address, course_id: u32) {
        // 1. caller.require_auth()
        caller.require_auth();

        // 2. Fetch 'Admin' (Registry) address from Instance storage. Assert caller == Admin.
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        assert!(
            caller == stored_admin,
            "Unauthorized: Caller is not the authorized registry"
        );

        // 3. Construct DataKey::UserBadges(learner).
        let badges_key = DataKey::UserBadges(learner.clone());

        // 4. Fetch existing Vec<Badge> or initialize empty Vec.
        let mut badges: Vec<Badge> = env
            .storage()
            .persistent()
            .get(&badges_key)
            .unwrap_or_else(|| Vec::new(&env));

        // 5. Check if badge with course_id exists (prevent duplicates).
        for existing_badge in badges.iter() {
            if existing_badge.course_id == course_id {
                panic!("Badge for this course already exists");
            }
        }

        // 6. Push new Badge to Vec and save to Persistent storage.
        let minted_at = env.ledger().timestamp();
        let new_badge = Badge {
            course_id,
            minted_at,
        };

        badges.push_back(new_badge);
        env.storage().persistent().set(&badges_key, &badges);

        // 7. Emit BadgeMinted event.
        BadgeMinted {
            learner,
            course_id,
            minted_at,
        }
        .publish(&env);
    }

    /// Returns all badges for a specific learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    ///
    /// # Returns
    /// Vector of Badge structs. Returns empty vector if learner has no badges.
    pub fn get_badges(env: Env, learner: Address) -> Vec<Badge> {
        let badges_key = DataKey::UserBadges(learner);
        env.storage()
            .persistent()
            .get(&badges_key)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the count of badges for a specific learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    ///
    /// # Returns
    /// Number of badges the learner owns.
    pub fn get_badge_count(env: Env, learner: Address) -> u32 {
        let badges = Self::get_badges(env, learner);
        badges.len()
    }

    /// Checks if a learner has a specific badge.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course ID to check
    ///
    /// # Returns
    /// true if the learner has the badge, false otherwise.
    pub fn has_badge(env: Env, learner: Address, course_id: u32) -> bool {
        let badges = Self::get_badges(env, learner);
        for badge in badges.iter() {
            if badge.course_id == course_id {
                return true;
            }
        }
        false
    }
}

mod test;
