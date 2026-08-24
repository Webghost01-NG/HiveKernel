use hive_core::{AgentId, PublicKeyHex, TaskId, TaskReceipt, TaskSpec};
use serde::{Deserialize, Serialize};

/// Wire messages exchanged across the HiveKernel P2P mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmMessage {
    /// Step 1: Delegator broadcasts task RFQ
    TaskRfq(TaskSpec),

    /// Step 2: Worker submits bid for the RFQ
    BidOffer {
        task_id: TaskId,
        worker_id: AgentId,
        worker_pubkey: PublicKeyHex,
        bid_bounty: u64,
        estimated_duration_ms: u64,
        reputation_score: u32,
    },

    /// Step 3: Delegator awards task to the winning bidder
    TaskAwarded {
        task_id: TaskId,
        assigned_worker: AgentId,
    },

    /// Step 4: Worker broadcasts signed completion receipt
    ReceiptBroadcast(TaskReceipt),

    /// Step 5: Validator raises an optimistic dispute / fraud challenge
    DisputeChallenge {
        task_id: TaskId,
        validator_id: AgentId,
        reason: String,
        challenger_stake: u64,
    },

    /// Step 6: Final settlement event (payout released or slashed)
    SettlementFinalized {
        task_id: TaskId,
        payout_recipient: AgentId,
        amount: u64,
        is_slashed: bool,
    },
}
