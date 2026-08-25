use crate::error::{HiveError, Result};
use crate::types::{AgentId, PublicKeyHex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: AgentId,
    pub public_key: PublicKeyHex,
    pub reputation_score: u32,
    pub tasks_completed: u64,
    pub tasks_slashed: u64,
}

/// Agent Registry: Cryptographically binds Agent IDs to verified Ed25519 public keys & tracks live reputation
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AgentRegistry {
    agents: HashMap<AgentId, AgentProfile>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn register_agent(
        &mut self,
        agent_id: impl Into<String>,
        public_key: impl Into<PublicKeyHex>,
    ) {
        let id = agent_id.into();
        if !self.agents.contains_key(&id) {
            self.agents.insert(
                id.clone(),
                AgentProfile {
                    agent_id: id,
                    public_key: public_key.into(),
                    reputation_score: 80, // Initial base reputation score
                    tasks_completed: 0,
                    tasks_slashed: 0,
                },
            );
        }
    }

    pub fn get_public_key(&self, agent_id: &str) -> Option<&PublicKeyHex> {
        self.agents.get(agent_id).map(|p| &p.public_key)
    }

    pub fn get_reputation(&self, agent_id: &str) -> u32 {
        self.agents
            .get(agent_id)
            .map(|p| p.reputation_score)
            .unwrap_or(50)
    }

    pub fn record_settlement(&mut self, agent_id: &str, success: bool) {
        if let Some(profile) = self.agents.get_mut(agent_id) {
            if success {
                profile.tasks_completed += 1;
                profile.reputation_score = (profile.reputation_score + 2).min(100);
            } else {
                profile.tasks_slashed += 1;
                profile.reputation_score = profile.reputation_score.saturating_sub(25);
            }
        }
    }

    pub fn verify_agent_identity(&self, agent_id: &str, provided_pubkey: &str) -> Result<()> {
        match self.agents.get(agent_id) {
            Some(profile) => {
                if profile.public_key == provided_pubkey {
                    Ok(())
                } else {
                    Err(HiveError::AgentError(format!(
                        "Public key spoofing detected: Agent '{}' registered with pubkey '{}', but provided '{}'",
                        agent_id, profile.public_key, provided_pubkey
                    )))
                }
            }
            None => Err(HiveError::AgentError(format!(
                "Agent '{}' is not registered in the Swarm identity registry",
                agent_id
            ))),
        }
    }
}
