use crate::types::{AgentCapability, AgentId, PublicKeyHex};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAgentNode {
    pub id: AgentId,
    pub public_key: PublicKeyHex,
    pub capabilities: Vec<AgentCapability>,
    pub reputation_score: u32,
}

impl SwarmAgentNode {
    pub fn new(
        id: impl Into<AgentId>,
        public_key: impl Into<PublicKeyHex>,
        capabilities: Vec<AgentCapability>,
    ) -> Self {
        Self {
            id: id.into(),
            public_key: public_key.into(),
            capabilities,
            reputation_score: 80,
        }
    }
}
