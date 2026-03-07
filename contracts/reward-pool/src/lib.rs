#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, token, Address, Env};

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

#[contractevent]
pub struct RewardDistributed {
    #[topic]
    pub caller: Address,
    #[topic]
    pub learner: Address,
    pub amount: i128,
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
    /// * If contract is not initialized
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

    /// Distributes rewards from the pool to a learner.
    ///
    /// # Arguments
    /// * `caller` - The spender contract address (must be whitelisted)
    /// * `learner` - The learner address to receive the reward
    /// * `amount` - The amount of tokens to transfer
    ///
    /// # Panics
    /// * If caller authentication fails
    /// * If amount is not positive
    /// * If caller is not an authorized spender
    /// * If contract is not initialized
    pub fn distribute_reward(env: Env, caller: Address, learner: Address, amount: i128) {
        // 1. caller.require_auth()
        caller.require_auth();

        // 2. Assert amount > 0
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        // 3. Construct DataKey::Spender(caller.clone())
        // 4. Fetch the boolean from Persistent storage. Assert it is true
        let is_authorized: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Spender(caller.clone()))
            .unwrap_or(false);

        if !is_authorized {
            panic!("Caller is not an authorized spender");
        }

        // 5. Fetch the 'Token' address from Instance storage
        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");

        // 6. Initialize token::Client::new(&env, &token_id)
        let token_client = token::Client::new(&env, &token_id);

        // 7. Call token_client.transfer(&env.current_contract_address(), &learner, &amount)
        token_client.transfer(&env.current_contract_address(), &learner, &amount);

        // 8. Emit RewardDistributed event
        RewardDistributed {
            caller,
            learner,
            amount,
        }
        .publish(&env);
    }
}

mod test;
