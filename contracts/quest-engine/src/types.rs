use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuestType {
    Build,
    Explore,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quest {
    pub employer: Address,
    pub reward_amount: i128,
    pub quest_type: QuestType,
    pub metadata_hash: BytesN<32>,
    pub active: bool,
    pub has_approved_submission: bool,
    // Total escrow deposited for this quest (0 for Explore quests, which are
    // paid from the RewardPool rather than a per-quest escrow).
    pub total_funded: i128,
    // Running total of funds already paid out from this quest's escrow
    // (fees + learner payouts, including any boost applied).
    pub consumed_amount: i128,
    // Running total of funds returned to the employer via refunds.
    pub refunded_amount: i128,
}

impl Quest {
    /// Escrow still available for future payouts or refunds.
    pub fn remaining_budget(&self) -> i128 {
        self.total_funded - self.consumed_amount - self.refunded_amount
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestBudget {
    pub total_funded: i128,
    pub consumed_amount: i128,
    pub refunded_amount: i128,
    pub remaining: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionStatus {
    Pending,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Submission {
    pub proof_hash: BytesN<32>,
    pub status: SubmissionStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Quest(u32),
    Submission(Address, u32), // (Submitter Address, Quest ID)
    Token,
    QuestCounter,
    RewardPool,
    IsPaused,
    StakeVault,
    Verified(Address, u32),
}
