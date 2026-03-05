#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
pub struct Course {
    pub active: bool,
    pub total_modules: u32,
}

#[contracttype]
pub enum DataKey {
    Course(u32),
    Progress(Address, u32),
}

#[contract]
pub struct CourseRegistry;

#[contractimpl]
impl CourseRegistry {
    /// Creates a new active course with the given id and total module count.
    pub fn create_course(env: Env, id: u32, total_modules: u32) {
        let course = Course {
            active: true,
            total_modules,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Course(id), &course);
    }

    /// Sets the active status of an existing course.
    pub fn set_active(env: Env, id: u32, active: bool) {
        let mut course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .unwrap();
        course.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Course(id), &course);
    }

    /// Enrolls a learner in an active course, initializing their progress to 0.
    ///
    /// Panics if the course does not exist, is not active, or the learner is
    /// already enrolled.
    pub fn enroll(env: Env, learner: Address, id: u32) {
        // 1. Authenticate the learner.
        learner.require_auth();

        // 2. Retrieve the Course struct; panics if it does not exist.
        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(id))
            .unwrap();

        // 3. Assert the course is active.
        assert!(course.active);

        // 4. Construct the Progress key.
        let progress_key = DataKey::Progress(learner, id);

        // 5. Assert the learner is not already enrolled.
        assert!(!env.storage().persistent().has(&progress_key));

        // 6. Initialize progress to 0.
        env.storage().persistent().set(&progress_key, &0u32);
    }

    /// Returns the number of completed modules for an enrolled learner.
    pub fn get_progress(env: Env, learner: Address, id: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Progress(learner, id))
            .unwrap()
    }
}

#[cfg(test)]
mod test;
