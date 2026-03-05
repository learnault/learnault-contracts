#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

/// CourseRegistry contract - Manages courses and their status on the Learnault platform.
/// Allows admins to deactivate courses to stop new enrollments while preserving
/// learner credentials.
#[contract]
pub struct CourseRegistry;

#[contractimpl]
impl CourseRegistry {
    /// Sets the active status of a course.
    ///
    /// Only the admin address can trigger this change. Deactivated courses remain
    /// in storage so past learners keep their credentials, but the state change
    /// signals the frontend to hide the course and blocks new enrollments.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `admin` - The admin address performing the action
    /// * `id` - The course ID to update
    /// * `active` - The new active status (true = active, false = deactivated)
    ///
    /// # Returns
    /// A symbol indicating success or error
    ///
    /// # Panics
    /// - If the caller is not the admin address
    /// - If the course does not exist
    pub fn set_course_status(env: Env, admin: Address, id: u32, active: bool) -> Symbol {
        // 1. Authenticate the admin
        admin.require_auth();

        // 2. Retrieve the course from persistent storage
        let course_key = (symbol_short!("course"), id);
        let _course: (u32, Symbol, bool) = env
            .storage()
            .persistent()
            .get(&course_key)
            .expect("Course not found");

        // 3. Assert course exists (already done by expect above)
        // 4. Update the active status
        let active_key = (symbol_short!("active"), id);
        env.storage().persistent().set(&active_key, &active);

        // 5. Emit CourseStatusChanged event
        let status_str = if active {
            symbol_short!("active")
        } else {
            symbol_short!("inactive")
        };

        env.events().publish(
            (symbol_short!("status"), id),
            (admin, status_str),
        );

        symbol_short!("success")
    }

    /// Creates a new course in the registry.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `admin` - The admin address performing the action
    /// * `id` - The unique course ID
    /// * `title` - The course title
    ///
    /// # Returns
    /// A symbol indicating success or error
    pub fn create_course(env: Env, admin: Address, id: u32, title: Symbol) -> Symbol {
        // Authenticate the admin
        admin.require_auth();

        // Store course data as tuple (id, title, active)
        let course_key = (symbol_short!("course"), id);
        let course = (id, title.clone(), true);
        env.storage().persistent().set(&course_key, &course);

        // Emit CourseCreated event
        env.events()
            .publish((symbol_short!("created"), id), (admin, title));

        symbol_short!("success")
    }

    /// Retrieves a course from the registry.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `id` - The course ID to retrieve
    ///
    /// # Returns
    /// A tuple of (id, title, active) or panics if course not found
    pub fn get_course(env: Env, id: u32) -> (u32, Symbol, bool) {
        let course_key = (symbol_short!("course"), id);
        let course: (u32, Symbol, bool) = env
            .storage()
            .persistent()
            .get(&course_key)
            .expect("Course not found");

        course
    }
}

#[cfg(test)]
mod test;
