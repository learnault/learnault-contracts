use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Course {
    pub instructor: Address,
    pub total_modules: u32,
    pub metadata_hash: BytesN<32>,
    pub active: bool,
    pub reward_amount: i128,
    pub completion_policy: CompletionPolicy, // <-- This line was missing
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompletionPolicy {
    /// No integrations required - all side effects silently skipped if addresses missing
    Optional,
    /// Reward pool required for course completion
    RewardRequired,
    /// Badge NFT required for course completion
    BadgeRequired,
    /// Both reward pool and badge NFT required for course completion
    BothRequired,
} 

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Course(u32),
    Progress(Address, u32),
    CourseCount,
    Admin,
    BadgeNftAddress,
    RewardPoolAddress,
    CompletionPolicy(u32),
}
