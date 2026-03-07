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

#[contractevent]
pub struct SpenderAdded {
    #[topic]
    pub spender: Address,
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

    /// Adds a contract address to the approved spender whitelist.
    ///
    /// # Arguments
    /// * `admin` - The admin address (must match stored admin)
    /// * `spender` - The contract address to whitelist
    ///
    /// # Panics
    /// * If admin does not match stored admin
    /// * If admin authentication fails
    pub fn add_approved_spender(env: Env, admin: Address, spender: Address) {
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

        // 4. Save `true` to Persistent storage using DataKey::Spender(spender.clone())
        env.storage()
            .persistent()
            .set(&DataKey::Spender(spender.clone()), &true);

        // 5. Emit SpenderAdded event
        SpenderAdded { spender }.publish(&env);
    }
}

mod test;
