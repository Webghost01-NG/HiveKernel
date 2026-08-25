use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type TaskId = Uuid;
pub type AgentId = String;
pub type PublicKeyHex = String;
pub type SignatureHex = String;
pub type ExecutionDigest = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentCapability {
    SmartContractAuditor,
    SecurityFuzzer,
    FormalVerifier,
    GasOptimizer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    PendingAuction,
    Assigned,
    Executing,
    AwaitingOptimisticValidation,
    Disputed,
    Settled,
    Slashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: TaskId,
    pub delegator: AgentId,
    pub delegator_pubkey: PublicKeyHex,
    pub capability_required: AgentCapability,
    pub description: String,
    pub input_payload: String,
    pub max_bounty: u64,
    pub max_execution_seconds: u64,
    pub challenge_window_seconds: u64,
    pub created_at: DateTime<Utc>,
}

impl TaskSpec {
    pub fn new(
        delegator: impl Into<AgentId>,
        delegator_pubkey: impl Into<PublicKeyHex>,
        capability_required: AgentCapability,
        description: impl Into<String>,
        input_payload: impl Into<String>,
        max_bounty: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            delegator: delegator.into(),
            delegator_pubkey: delegator_pubkey.into(),
            capability_required,
            description: description.into(),
            input_payload: input_payload.into(),
            max_bounty,
            max_execution_seconds: 30,
            challenge_window_seconds: 15,
            created_at: Utc::now(),
        }
    }
}
