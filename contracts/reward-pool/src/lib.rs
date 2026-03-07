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
pub struct PoolFunded {
    #[topic]
    pub donor: Address,
    pub amount: i128,
}

#[contractimpl]
impl RewardPool {
    pub fn initialize(env: Env, admin: Address, token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);

        env.storage().instance().set(&DataKey::Token, &token);

        PoolInitialized { admin, token }.publish(&env);
    }

    pub fn fund_pool(env: Env, donor: Address, amount: i128) {
        donor.require_auth();
        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&donor, &env.current_contract_address(), &amount);
        PoolFunded { donor, amount }.publish(&env);
    }
}

mod test;
