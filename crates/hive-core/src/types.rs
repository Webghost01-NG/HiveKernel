use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an agent on the network
pub type AgentId = String;

/// Unique identifier for an outsourced task
pub type TaskId = Uuid;

/// Hex-encoded cryptographic public key (Ed25519)
pub type PublicKeyHex = String;

/// Specialized capabilities offered by an agent in the swarm
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentCapability {
    SmartContractAuditor,
    DeFiRiskAnalyzer,
    OracleValidator,
    DataScraper,
    ZeroKnowledgeProver,
    GeneralLLMWorker,
}

/// Status lifecycle of an outsourced agent task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    PendingAuction,
    Assigned,
    Executing,
    AwaitingOptimisticValidation,
    Disputed,
    Settled,
    Slashed,
}

/// Task specification broadcast by a Delegator Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: TaskId,
    pub delegator: AgentId,
    pub delegator_pubkey: PublicKeyHex,
    pub required_capability: AgentCapability,
    pub description: String,
    pub input_payload: String,
    pub bounty_amount: u64, // In mock USDC / native units
    pub max_execution_seconds: u64,
    pub challenge_window_seconds: u64,
    pub created_at: DateTime<Utc>,
}

impl TaskSpec {
    pub fn new(
        delegator: impl Into<String>,
        delegator_pubkey: impl Into<String>,
        required_capability: AgentCapability,
        description: impl Into<String>,
        input_payload: impl Into<String>,
        bounty_amount: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            delegator: delegator.into(),
            delegator_pubkey: delegator_pubkey.into(),
            required_capability,
            description: description.into(),
            input_payload: input_payload.into(),
            bounty_amount,
            max_execution_seconds: 30,
            challenge_window_seconds: 15,
            created_at: Utc::now(),
        }
    }
}
