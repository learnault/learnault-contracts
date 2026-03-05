#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

/// Course struct representing a course in the registry.
/// 
/// # Fields
/// * `instructor` - The address of the course instructor who can modify the course
/// * `metadata_hash` - The IPFS hash (32 bytes) pointing to off-chain course content
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Course {
    pub instructor: Address,
    pub metadata_hash: BytesN<32>,
}

/// Storage keys for the contract data.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Key for storing a Course by its ID
    Course(u32),
}

/// Event emitted when a course's metadata hash is updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataUpdated {
    pub course_id: u32,
    pub new_hash: BytesN<32>,
    pub updated_by: Address,
}

/// CourseRegistry contract - Manages course metadata on the Learnault platform.
/// 
/// This contract allows instructors to register courses and update their metadata
/// while maintaining on-chain course IDs for enrolled learners.
#[contract]
pub struct CourseRegistry;

#[contractimpl]
impl CourseRegistry {
    /// Updates the metadata hash for an existing course.
    /// 
    /// This function allows the designated instructor of a course to update the IPFS hash
    /// pointing to the off-chain content. This is crucial for fixing typos or updating
    /// outdated curriculum while maintaining the enrolled learners' progress.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `id` - The unique identifier of the course to update
    /// * `new_hash` - The new IPFS hash (32 bytes) for the course metadata
    ///
    /// # Returns
    /// Nothing on success
    ///
    /// # Panics
    /// * If the course with the given ID does not exist
    /// * If the caller is not the instructor of the course (authentication failure)
    ///
    /// # Events
    /// Emits a `MetadataUpdated` event containing the course ID, new hash, and updater address
    pub fn update_metadata(env: Env, id: u32, new_hash: BytesN<32>) {
        // 1. Retrieve Course struct from Persistent storage using DataKey::Course(id)
        let key = DataKey::Course(id);
        let mut course: Course = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Course not found");

        // 2. Authenticate the stored instructor
        course.instructor.require_auth();

        // 3. Update course.metadata_hash
        course.metadata_hash = new_hash.clone();

        // 4. Save updated Course struct back to Persistent storage
        env.storage().persistent().set(&key, &course);

        // 5. Emit MetadataUpdated event
        let event = MetadataUpdated {
            course_id: id,
            new_hash,
            updated_by: course.instructor.clone(),
        };
        env.events().publish(("MetadataUpdated",), event);
    }

    /// Creates a new course in the registry.
    /// 
    /// This is a helper function for testing and course creation.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `id` - The unique identifier for the new course
    /// * `instructor` - The address of the course instructor
    /// * `metadata_hash` - The initial IPFS hash for the course metadata
    ///
    /// # Panics
    /// * If a course with the given ID already exists
    pub fn create_course(
        env: Env,
        id: u32,
        instructor: Address,
        metadata_hash: BytesN<32>,
    ) {
        instructor.require_auth();

        let key = DataKey::Course(id);
        
        // Check if course already exists
        if env.storage().persistent().has(&key) {
            panic!("Course already exists");
        }

        let course = Course {
            instructor,
            metadata_hash,
        };

        env.storage().persistent().set(&key, &course);
    }

    /// Retrieves a course by its ID.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `id` - The unique identifier of the course
    ///
    /// # Returns
    /// The Course struct containing instructor and metadata hash
    ///
    /// # Panics
    /// * If the course with the given ID does not exist
    pub fn get_course(env: Env, id: u32) -> Course {
        let key = DataKey::Course(id);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Course not found")
    }
}

#[cfg(test)]
mod test;
