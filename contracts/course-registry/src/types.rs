use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Course {
    pub instructor: Address,
    pub total_modules: u32,
    pub metadata_hash: BytesN<32>,
    pub active: bool,
    pub reward_amount: i128,
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
}
