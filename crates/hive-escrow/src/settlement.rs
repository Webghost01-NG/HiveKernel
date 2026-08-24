use hive_core::types::AgentId;
use std::collections::HashMap;

/// Balance Ledger for Swarm Agent Accounts
#[derive(Debug, Default)]
pub struct SwarmLedger {
    balances: HashMap<AgentId, u64>,
}

impl SwarmLedger {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
        }
    }

    pub fn deposit(&mut self, agent: &AgentId, amount: u64) {
        *self.balances.entry(agent.clone()).or_insert(0) += amount;
    }

    pub fn withdraw(&mut self, agent: &AgentId, amount: u64) -> bool {
        let bal = self.balances.entry(agent.clone()).or_insert(0);
        if *bal >= amount {
            *bal -= amount;
            true
        } else {
            false
        }
    }

    pub fn transfer(&mut self, from: &AgentId, to: &AgentId, amount: u64) -> bool {
        if self.withdraw(from, amount) {
            self.deposit(to, amount);
            true
        } else {
            false
        }
    }

    pub fn balance_of(&self, agent: &AgentId) -> u64 {
        *self.balances.get(agent).unwrap_or(&0)
    }
}
