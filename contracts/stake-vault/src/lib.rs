#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, token, Address, BytesN, Env, Vec};

pub mod types;
use types::{default_multiplier_tiers, DataKey, MultiplierTier, StakeInfo};

#[contract]
pub struct StakeVault;

#[contractevent]
pub struct StakeVaultInitialized {
    #[topic]
    pub admin: Address,
    #[topic]
    pub token: Address,
}

#[contractevent]
pub struct Staked {
    #[topic]
    pub user: Address,
    pub amount: i128,
    pub total_staked: i128,
    pub lock_timestamp: u64,
}

#[contractevent]
pub struct Unstaked {
    #[topic]
    pub user: Address,
    pub amount: i128,
}

#[contractevent]
pub struct MultiplierTiersUpdated {
    #[topic]
    pub admin: Address,
    pub tiers: Vec<MultiplierTier>,
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
}

fn validate_tiers(tiers: &Vec<MultiplierTier>) {
    let mut prev_min_stake: Option<i128> = None;
    for tier in tiers.iter() {
        if tier.min_stake <= 0 {
            panic!("Tier min_stake must be positive");
        }
        if tier.multiplier == 0 {
            panic!("Tier multiplier must be non-zero");
        }
        if let Some(prev) = prev_min_stake {
            if tier.min_stake >= prev {
                panic!("Tier min_stake must be strictly descending");
            }
        }
        prev_min_stake = Some(tier.min_stake);
    }
}

#[contractimpl]
impl StakeVault {
    pub fn initialize(env: Env, admin: Address, token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);

        let default_tiers = default_multiplier_tiers(&env);
        env.storage()
            .instance()
            .set(&DataKey::MultiplierTiers, &default_tiers);

        StakeVaultInitialized { admin, token }.publish(&env);
    }

    pub fn set_multiplier_tiers(env: Env, admin: Address, tiers: Vec<MultiplierTier>) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if admin != stored_admin {
            panic!("Unauthorized");
        }

        validate_tiers(&tiers);

        env.storage()
            .instance()
            .set(&DataKey::MultiplierTiers, &tiers);

        MultiplierTiersUpdated { admin, tiers }.publish(&env);
    }

    pub fn get_multiplier_tiers(env: Env) -> Vec<MultiplierTier> {
        env.storage()
            .instance()
            .get(&DataKey::MultiplierTiers)
            .unwrap_or_else(|| default_multiplier_tiers(&env))
    }

    pub fn stake(env: Env, user: Address, amount: i128) {
        user.require_auth();

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_id);

        token_client.transfer(&user, env.current_contract_address(), &amount);

        let now = env.ledger().timestamp();

        let mut stake_info: StakeInfo = env
            .storage()
            .persistent()
            .get(&DataKey::UserStake(user.clone()))
            .unwrap_or(StakeInfo {
                amount: 0,
                lock_timestamp: now,
            });

        stake_info.amount += amount;
        stake_info.lock_timestamp = now;

        env.storage()
            .persistent()
            .set(&DataKey::UserStake(user.clone()), &stake_info);

        Staked {
            user,
            amount,
            total_staked: stake_info.amount,
            lock_timestamp: stake_info.lock_timestamp,
        }
        .publish(&env);
    }

    pub fn unstake(env: Env, user: Address) {
        user.require_auth();

        let stake_info: StakeInfo = env
            .storage()
            .persistent()
            .get(&DataKey::UserStake(user.clone()))
            .expect("No stake found");

        let lock_period: u64 = 604800;
        if env.ledger().timestamp() < stake_info.lock_timestamp + lock_period {
            panic!("Lock period active");
        }

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_id);

        token_client.transfer(
            &env.current_contract_address(),
            user.clone(),
            &stake_info.amount,
        );

        env.storage()
            .persistent()
            .remove(&DataKey::UserStake(user.clone()));

        Unstaked {
            user,
            amount: stake_info.amount,
        }
        .publish(&env);
    }

    pub fn get_multiplier(env: Env, user: Address) -> u32 {
        let stake_info: StakeInfo = env
            .storage()
            .persistent()
            .get(&DataKey::UserStake(user))
            .unwrap_or(StakeInfo {
                amount: 0,
                lock_timestamp: 0,
            });

        let tiers: Vec<MultiplierTier> = env
            .storage()
            .instance()
            .get(&DataKey::MultiplierTiers)
            .unwrap_or_else(|| default_multiplier_tiers(&env));

        for tier in tiers.iter() {
            if stake_info.amount >= tier.min_stake {
                return tier.multiplier;
            }
        }

        100
    }

    pub fn upgrade_contract(env: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if admin != stored_admin {
            panic!("Unauthorized");
        }

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
