#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Env};

pub mod types;

pub use types::{Course, DataKey};

/// Course registry contract: courses and learner progress.
#[contract]
pub struct CourseRegistry;

#[contractimpl]
impl CourseRegistry {
    /// Returns the completed module count for a learner in a course.
    /// Returns 0 if the learner has not enrolled or no progress is stored.
    pub fn get_progress(env: Env, learner: Address, id: u32) -> u32 {
        let key = DataKey::Progress(learner, id);
        env.storage().persistent().get(&key).unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
