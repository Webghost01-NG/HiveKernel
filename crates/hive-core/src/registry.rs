use crate::error::{HiveError, Result};
use crate::types::{AgentId, PublicKeyHex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent Registry: Cryptographically binds Agent IDs to verified Ed25519 public keys
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AgentRegistry {
    agents: HashMap<AgentId, PublicKeyHex>,
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
        self.agents.insert(agent_id.into(), public_key.into());
    }

    pub fn get_public_key(&self, agent_id: &str) -> Option<&PublicKeyHex> {
        self.agents.get(agent_id)
    }

    pub fn verify_agent_identity(&self, agent_id: &str, provided_pubkey: &str) -> Result<()> {
        match self.agents.get(agent_id) {
            Some(registered_pubkey) => {
                if registered_pubkey == provided_pubkey {
                    Ok(())
                } else {
                    Err(HiveError::AgentError(format!(
                        "Public key spoofing detected: Agent '{}' registered with pubkey '{}', but provided '{}'",
                        agent_id, registered_pubkey, provided_pubkey
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
