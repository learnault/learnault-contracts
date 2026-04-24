#![no_std]

pub mod types;

// Added contractimpl to the import list here
use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct QuestEngineContract;

// We will implement contract functions in future issues.
#[contractimpl]
impl QuestEngineContract {
    // Empty for now to satisfy the compiler
}
