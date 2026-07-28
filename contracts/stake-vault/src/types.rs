use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeInfo {
    pub amount: i128,
    pub lock_timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiplierTier {
    pub min_stake: i128,
    pub multiplier: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Token,
    UserStake(Address),
    MultiplierTiers,
}

pub fn default_multiplier_tiers(env: &Env) -> Vec<MultiplierTier> {
    Vec::from_array(
        env,
        [
            MultiplierTier {
                min_stake: 500,
                multiplier: 200,
            },
            MultiplierTier {
                min_stake: 100,
                multiplier: 120,
            },
        ],
    )
}
