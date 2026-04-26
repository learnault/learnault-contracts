#![no_std]

pub mod types;
use types::{DataKey, Quest, QuestType};

use soroban_sdk::{
    contract, contractevent, contractimpl, token, Address, BytesN, Env,
};

#[contractevent]
pub struct QuestCreated {
    #[topic]
    pub employer: Address,
    #[topic]
    pub quest_id: u32,
    pub reward_amount: i128,
}

#[contract]
pub struct QuestEngineContract;

#[contractimpl]
impl QuestEngineContract {
    /// Initializes the QuestEngine contract with the token address.
    pub fn initialize(env: Env, token: Address) {
        if env.storage().instance().has(&DataKey::Token) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::QuestCounter, &0u32);
    }

    /// Allows an employer to lock USDC directly in the QuestEngine contract.
    /// This acts as an isolated vault specifically for B2B bounties.
    pub fn create_build_quest(
        env: Env,
        employer: Address,
        reward_amount: i128,
        metadata_hash: BytesN<32>,
    ) -> u32 {
        // 1. employer.require_auth()
        employer.require_auth();

        // 2. Fetch token_client for the USDC asset.
        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Not initialized");
        let token_client = token::Client::new(&env, &token_address);

        // 3. call token_client.transfer(employer, env.current_contract_address(), reward_amount).
        token_client.transfer(&employer, &env.current_contract_address(), &reward_amount);

        // 4. Increment Quest ID counter.
        let mut quest_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::QuestCounter)
            .unwrap_or(0);
        quest_id += 1;
        env.storage().instance().set(&DataKey::QuestCounter, &quest_id);

        // 5. Create Quest struct with QuestType::Build.
        let quest = Quest {
            employer: employer.clone(),
            reward_amount,
            quest_type: QuestType::Build,
            metadata_hash,
            active: true,
        };

        // 6. Save to Persistent storage.
        env.storage().persistent().set(&DataKey::Quest(quest_id), &quest);

        // 7. Emit QuestCreated event.
        QuestCreated {
            employer,
            quest_id,
            reward_amount,
        }
        .publish(&env);

        quest_id
    }

    /// Returns a quest by its ID.
    pub fn get_quest(env: Env, quest_id: u32) -> Option<Quest> {
        env.storage().persistent().get(&DataKey::Quest(quest_id))
    }
}

mod test;
