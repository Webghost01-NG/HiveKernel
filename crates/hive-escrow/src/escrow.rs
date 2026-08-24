use chrono::{DateTime, Duration, Utc};
use hive_core::{
    error::{HiveError, Result},
    receipt::TaskReceipt,
    types::{AgentId, TaskId, TaskStatus},
};
use std::collections::HashMap;

/// Escrow Record for an active task
#[derive(Debug, Clone)]
pub struct EscrowRecord {
    pub task_id: TaskId,
    pub delegator: AgentId,
    pub assigned_worker: Option<AgentId>,
    pub bounty_amount: u64,
    pub status: TaskStatus,
    pub challenge_deadline: Option<DateTime<Utc>>,
    pub receipt: Option<TaskReceipt>,
    pub dispute_reason: Option<String>,
}

/// In-Memory Optimistic Escrow Manager
pub struct EscrowManager {
    records: HashMap<TaskId, EscrowRecord>,
}

impl EscrowManager {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    pub fn lock_escrow(&mut self, task_id: TaskId, delegator: AgentId, bounty: u64) -> Result<()> {
        let record = EscrowRecord {
            task_id,
            delegator,
            assigned_worker: None,
            bounty_amount: bounty,
            status: TaskStatus::PendingAuction,
            challenge_deadline: None,
            receipt: None,
            dispute_reason: None,
        };
        self.records.insert(task_id, record);
        Ok(())
    }

    pub fn assign_worker(&mut self, task_id: TaskId, worker: AgentId) -> Result<()> {
        let record = self.records.get_mut(&task_id)
            .ok_or_else(|| HiveError::EscrowError("Escrow record not found".to_string()))?;
        
        record.assigned_worker = Some(worker);
        record.status = TaskStatus::Assigned;
        Ok(())
    }

    pub fn submit_receipt(&mut self, receipt: TaskReceipt, challenge_window_seconds: u64) -> Result<()> {
        let task_id = receipt.task_id;
        let record = self.records.get_mut(&task_id)
            .ok_or_else(|| HiveError::EscrowError("Escrow record not found".to_string()))?;

        let deadline = Utc::now() + Duration::seconds(challenge_window_seconds as i64);
        record.receipt = Some(receipt);
        record.challenge_deadline = Some(deadline);
        record.status = TaskStatus::AwaitingOptimisticValidation;
        Ok(())
    }

    pub fn raise_dispute(&mut self, task_id: TaskId, reason: String) -> Result<()> {
        let record = self.records.get_mut(&task_id)
            .ok_or_else(|| HiveError::EscrowError("Escrow record not found".to_string()))?;

        record.status = TaskStatus::Disputed;
        record.dispute_reason = Some(reason);
        Ok(())
    }

    pub fn finalize_settlement(&mut self, task_id: TaskId) -> Result<TaskStatus> {
        let record = self.records.get_mut(&task_id)
            .ok_or_else(|| HiveError::EscrowError("Escrow record not found".to_string()))?;

        if record.status == TaskStatus::Disputed {
            record.status = TaskStatus::Slashed;
            return Ok(TaskStatus::Slashed);
        }

        record.status = TaskStatus::Settled;
        Ok(TaskStatus::Settled)
    }

    pub fn get_record(&self, task_id: &TaskId) -> Option<&EscrowRecord> {
        self.records.get(task_id)
    }
}
