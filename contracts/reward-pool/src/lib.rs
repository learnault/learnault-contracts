#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, Address, Env};

pub mod types;
use types::DataKey;

#[contract]
pub struct RewardPool;

#[contractevent]
pub struct PoolInitialized {
    #[topic]
    pub admin: Address,
    #[topic]
    pub token: Address,
}

#[contractimpl]
impl RewardPool {
    /// Initializes the RewardPool contract with admin and token addresses.
    ///
    /// # Arguments
    /// * `admin` - The admin address that will have administrative control
    /// * `token` - The SAC token address to be used as reward token
    ///
    /// # Panics
    /// * If contract is already initialized
    /// * If admin authentication fails
    pub fn initialize(env: Env, admin: Address, token: Address) {
        // 1. Check if already initialized
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        // 2. Require admin authentication
        admin.require_auth();

        // 3. Store admin in Instance storage
        env.storage().instance().set(&DataKey::Admin, &admin);

        // 4. Store token in Instance storage
        env.storage().instance().set(&DataKey::Token, &token);

        // 5. Emit PoolInitialized event
        PoolInitialized { admin, token }.publish(&env);
    }
}

mod test;
