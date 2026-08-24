use crate::types::{AgentCapability, AgentId, PublicKeyHex};
use serde::{Deserialize, Serialize};

/// Profile and metadata for an agent active in the HiveKernel swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: AgentId,
    pub pubkey: PublicKeyHex,
    pub capabilities: Vec<AgentCapability>,
    pub reputation_score: u32,
    pub total_tasks_completed: u64,
    pub total_earnings: u64,
    pub stake_deposit: u64,
}

impl AgentProfile {
    pub fn new(
        id: impl Into<String>,
        pubkey: impl Into<String>,
        capabilities: Vec<AgentCapability>,
        stake_deposit: u64,
    ) -> Self {
        Self {
            id: id.into(),
            pubkey: pubkey.into(),
            capabilities,
            reputation_score: 100, // Starts with base score
            total_tasks_completed: 0,
            total_earnings: 0,
            stake_deposit,
        }
    }
}
