#![no_std]
use soroban_sdk::{contractclient, contractevent, Address, BytesN, Env};

pub mod types;

#[contractclient(name = "RewardPoolClient")]
pub trait RewardPoolInterface {
    fn initialize(env: Env, admin: Address, token: Address);
    fn add_approved_spender(env: Env, admin: Address, spender: Address);
    fn remove_approved_spender(env: Env, admin: Address, spender: Address);
    fn set_pause(env: Env, admin: Address, status: bool);
    fn distribute_reward(env: Env, caller: Address, learner: Address, amount: i128);
    fn fund_pool(env: Env, donor: Address, amount: i128);
    fn emergency_sweep(env: Env, admin: Address, recovery_wallet: Address);
    fn upgrade_contract(env: Env, admin: Address, new_wasm_hash: BytesN<32>);
}

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
pub struct SpenderRemoved {
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

#[contractevent]
pub struct PoolFunded {
    #[topic]
    pub donor: Address,
    pub amount: i128,
}

#[contractevent]
pub struct EmergencySweep {
    #[topic]
    pub admin: Address,
    #[topic]
    pub recovery_wallet: Address,
    pub amount: i128,
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
}

#[cfg(feature = "contract")]
mod contract_impl {
    use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env};

    use crate::types::DataKey;
    use crate::{
        ContractUpgraded, EmergencySweep, PoolFunded, PoolInitialized, RewardDistributed,
        SpenderAdded, SpenderRemoved,
    };

    #[contract]
    pub struct RewardPool;

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

        /// Removes a contract address from the approved spender whitelist.
        ///
        /// # Arguments
        /// * `admin` - The admin address (must match stored admin)
        /// * `spender` - The contract address to remove from the whitelist
        ///
        /// # Panics
        /// * If contract is not initialized
        /// * If admin does not match stored admin
        /// * If admin authentication fails
        pub fn remove_approved_spender(env: Env, admin: Address, spender: Address) {
            let stored_admin: Address = env
                .storage()
                .instance()
                .get(&DataKey::Admin)
                .expect("Not initialized");

            if admin != stored_admin {
                panic!("Unauthorized");
            }

            admin.require_auth();

            env.storage()
                .persistent()
                .set(&DataKey::Spender(spender.clone()), &false);

            SpenderRemoved { spender }.publish(&env);
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
            // 0. Check if contract is paused
            let is_paused: bool = env
                .storage()
                .instance()
                .get(&DataKey::IsPaused)
                .unwrap_or(false);
            assert!(!is_paused, "Contract is paused");

            // 1. caller.require_auth()
            caller.require_auth();

            // 2. Assert amount > 0
            if amount <= 0 {
                panic!("Amount must be positive");
            }

            // 3. Check if contract is initialized first
            let token_id: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Not initialized");

            // 4. Construct DataKey::Spender(caller.clone())
            // 5. Fetch the boolean from Persistent storage. Assert it is true
            let is_authorized: bool = env
                .storage()
                .persistent()
                .get(&DataKey::Spender(caller.clone()))
                .unwrap_or(false);

            if !is_authorized {
                panic!("Caller is not an authorized spender");
            }

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

        /// Funds the reward pool with tokens from a donor.
        ///
        /// # Arguments
        /// * `donor` - The address donating the tokens
        /// * `amount` - The amount of tokens to donate
        ///
        /// # Panics
        /// * If contract is not initialized
        /// * If donor authentication fails
        /// * If token transfer fails
        pub fn fund_pool(env: Env, donor: Address, amount: i128) {
            // 1. donor.require_auth()
            donor.require_auth();

            // 2. Fetch 'Token_Address' from Instance storage
            let token_id: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Not initialized");

            // 3. Initialize token::Client::new(&env, &Token_Address)
            let token_client = token::Client::new(&env, &token_id);

            // 4. Call token_client.transfer(&donor, &env.current_contract_address(), &amount)
            token_client.transfer(&donor, env.current_contract_address(), &amount);

            // 5. Emit PoolFunded event
            PoolFunded { donor, amount }.publish(&env);
        }

        /// Emergency sweep function allowing admin to transfer all tokens from the contract
        /// to a recovery wallet in case of a critical vulnerability.
        ///
        /// # Arguments
        /// * `admin` - The admin address (must match stored admin)
        /// * `recovery_wallet` - The address to receive the swept tokens
        ///
        /// # Panics
        /// * If contract is not initialized
        /// * If admin does not match stored admin
        /// * If admin authentication fails
        pub fn emergency_sweep(env: Env, admin: Address, recovery_wallet: Address) {
            // 1. admin.require_auth()
            admin.require_auth();

            // 2. Fetch stored admin from Instance storage
            let stored_admin: Address = env
                .storage()
                .instance()
                .get(&DataKey::Admin)
                .expect("Not initialized");

            // 3. Assert admin == stored_admin
            if admin != stored_admin {
                panic!("Unauthorized");
            }

            // 4. Fetch token address from Instance storage
            let token_id: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Not initialized");

            // 5. Initialize token client
            let token_client = token::Client::new(&env, &token_id);

            // 6. Fetch full contract token balance
            let balance = token_client.balance(&env.current_contract_address());

            // 7. Transfer full balance to recovery wallet
            token_client.transfer(&env.current_contract_address(), &recovery_wallet, &balance);

            // 8. Emit EmergencySweep event
            EmergencySweep {
                admin,
                recovery_wallet,
                amount: balance,
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
    }
}

#[cfg(feature = "contract")]
pub use contract_impl::RewardPool;

mod test;
