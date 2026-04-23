#![no_std]

pub mod types;

use soroban_sdk::contract;

#[contract]
pub struct QuestEngineContract;

// We will implement contract functions in future issues.
#[contractimpl]
impl QuestEngineContract {
    // Empty for now to satisfy the compiler
}
