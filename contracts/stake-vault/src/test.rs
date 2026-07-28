#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, Vec,
};

use crate::{
    types::{DataKey, MultiplierTier, StakeInfo},
    StakeVault, StakeVaultClient,
};

fn setup() -> (Env, StakeVaultClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StakeVault, ());
    let client = StakeVaultClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_stake_transfers_and_accumulates() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_client = token::StellarAssetClient::new(&env, &token_id.address());

    client.initialize(&admin, &token_id.address());

    token_client.mint(&user, &1000);

    env.ledger().set_timestamp(1_000_000);
    client.stake(&user, &100);

    assert_eq!(token_client.balance(&user), 900);
    assert_eq!(token_client.balance(&client.address), 100);

    env.ledger().set_timestamp(1_000_100);
    client.stake(&user, &50);

    assert_eq!(token_client.balance(&user), 850);
    assert_eq!(token_client.balance(&client.address), 150);

    env.ledger().set_timestamp(1_000_100 + 604800);
    client.unstake(&user);

    assert_eq!(token_client.balance(&user), 1000);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
#[should_panic(expected = "Lock period active")]
fn test_stake_resets_lock_timestamp() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_client = token::StellarAssetClient::new(&env, &token_id.address());

    client.initialize(&admin, &token_id.address());
    token_client.mint(&user, &1000);

    env.ledger().set_timestamp(10_000);
    client.stake(&user, &100);

    env.ledger().set_timestamp(10_100);
    client.stake(&user, &50);

    env.ledger().set_timestamp(10_000 + 604800);
    client.unstake(&user);
}

#[test]
#[should_panic(expected = "Lock period active")]
fn test_unstake_panics_when_lock_period_active() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_client = token::StellarAssetClient::new(&env, &token_id.address());

    client.initialize(&admin, &token_id.address());
    token_client.mint(&user, &1000);

    env.ledger().set_timestamp(2_000_000);
    client.stake(&user, &100);

    env.ledger().set_timestamp(2_000_000 + 604799);
    client.unstake(&user);
}

#[test]
fn test_unstake_succeeds_after_lock() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_client = token::StellarAssetClient::new(&env, &token_id.address());

    client.initialize(&admin, &token_id.address());
    token_client.mint(&user, &1000);

    env.ledger().set_timestamp(3_000_000);
    client.stake(&user, &250);

    env.ledger().set_timestamp(3_000_000 + 604800);
    client.unstake(&user);

    assert_eq!(token_client.balance(&user), 1000);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
#[should_panic(expected = "No stake found")]
fn test_unstake_twice_panics_after_withdrawal() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_client = token::StellarAssetClient::new(&env, &token_id.address());

    client.initialize(&admin, &token_id.address());
    token_client.mint(&user, &1000);

    env.ledger().set_timestamp(4_000_000);
    client.stake(&user, &100);

    env.ledger().set_timestamp(4_000_000 + 604800);
    client.unstake(&user);

    client.unstake(&user);
}

#[test]
fn test_get_multiplier() {
    let (env, client) = setup();
    let user = Address::generate(&env);

    assert_eq!(client.get_multiplier(&user), 100);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 50,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 100);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 100,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 120);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 499,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 120);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 500,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 200);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 1000,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 200);
}

#[test]
fn test_set_multiplier_tiers_success() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let user = Address::generate(&env);

    client.initialize(&admin, &token_id.address());

    let new_tiers = Vec::from_array(
        &env,
        [
            MultiplierTier {
                min_stake: 1000,
                multiplier: 300,
            },
            MultiplierTier {
                min_stake: 200,
                multiplier: 150,
            },
        ],
    );

    client.set_multiplier_tiers(&admin, &new_tiers);
    assert_eq!(client.get_multiplier_tiers(), new_tiers);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 150,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 100);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 200,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 150);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::UserStake(user.clone()),
            &StakeInfo {
                amount: 1000,
                lock_timestamp: 0,
            },
        );
    });
    assert_eq!(client.get_multiplier(&user), 300);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_set_multiplier_tiers_unauthorized() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());

    client.initialize(&admin, &token_id.address());

    let new_tiers = Vec::from_array(
        &env,
        [MultiplierTier {
            min_stake: 500,
            multiplier: 200,
        }],
    );

    client.set_multiplier_tiers(&non_admin, &new_tiers);
}

#[test]
#[should_panic(expected = "Tier min_stake must be strictly descending")]
fn test_set_multiplier_tiers_invalid_order() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());

    client.initialize(&admin, &token_id.address());

    let invalid_tiers = Vec::from_array(
        &env,
        [
            MultiplierTier {
                min_stake: 100,
                multiplier: 120,
            },
            MultiplierTier {
                min_stake: 500,
                multiplier: 200,
            },
        ],
    );

    client.set_multiplier_tiers(&admin, &invalid_tiers);
}

#[test]
#[should_panic(expected = "Tier min_stake must be positive")]
fn test_set_multiplier_tiers_invalid_min_stake() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());

    client.initialize(&admin, &token_id.address());

    let invalid_tiers = Vec::from_array(
        &env,
        [MultiplierTier {
            min_stake: 0,
            multiplier: 150,
        }],
    );

    client.set_multiplier_tiers(&admin, &invalid_tiers);
}

#[test]
#[should_panic(expected = "Tier multiplier must be non-zero")]
fn test_set_multiplier_tiers_invalid_multiplier() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());

    client.initialize(&admin, &token_id.address());

    let invalid_tiers = Vec::from_array(
        &env,
        [MultiplierTier {
            min_stake: 500,
            multiplier: 0,
        }],
    );

    client.set_multiplier_tiers(&admin, &invalid_tiers);
}
