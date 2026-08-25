use chrono::{Duration, Utc};
use hive_core::{
    auditor::ContractAuditor,
    receipt::{AgentKeypair, TaskReceipt},
    registry::AgentRegistry,
    types::{AgentCapability, TaskSpec, TaskStatus},
};
use hive_escrow::{escrow::EscrowManager, settlement::SwarmLedger, verifier::SwarmVerifier};
use hive_p2p::auction::{AuctionMatcher, CandidateBid};

#[tokio::test]
async fn test_full_optimistic_lifecycle_success() {
    let mut ledger = SwarmLedger::new();
    let mut escrow = EscrowManager::new();
    let mut registry = AgentRegistry::new();

    let delegator = "Agent_Delegator".to_string();
    let worker = "Agent_Worker_Beta".to_string();
    let worker_key = AgentKeypair::generate();
    let delegator_key = AgentKeypair::generate();

    registry.register_agent(&delegator, delegator_key.public_key_hex());
    registry.register_agent(&worker, worker_key.public_key_hex());

    ledger.deposit(&delegator, 500);
    assert_eq!(ledger.balance_of(&delegator), 500);

    let task = TaskSpec::new(
        delegator.clone(),
        delegator_key.public_key_hex(),
        AgentCapability::SmartContractAuditor,
        "Staking.sol",
        "contract Staking {}",
        100,
    );

    // 1. Lock Escrow
    assert!(ledger.withdraw(&delegator, 100));
    assert!(escrow.lock_escrow(task.id, delegator.clone(), 100).is_ok());

    // 2. Bid and Assign
    let bid = CandidateBid {
        worker_id: worker.clone(),
        bid_bounty: 100,
        estimated_duration_ms: 250,
        reputation_score: 95,
    };
    let winning_bid = AuctionMatcher::select_best_bid(&[bid], 100).unwrap();
    assert_eq!(winning_bid.worker_id, worker);
    assert!(escrow.assign_worker(task.id, worker.clone()).is_ok());

    // 3. Worker executes & signs with full cryptographic binding
    let report = ContractAuditor::audit_source(&task.description, &task.input_payload);
    let output_json = serde_json::to_string(&report).unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        worker.clone(),
        &worker_key,
        &task.input_payload,
        output_json,
        250,
    );

    assert!(escrow.submit_receipt(receipt.clone(), 10).is_ok());

    // 4. Validator verifies
    let is_valid =
        SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry)).unwrap();
    assert!(is_valid);

    // 5. Finalize settlement after challenge window
    let after_deadline = Utc::now() + Duration::seconds(15);
    let final_status = escrow
        .finalize_settlement_at(task.id, after_deadline)
        .unwrap();
    assert_eq!(final_status, TaskStatus::Settled);

    ledger.deposit(&worker, 100);
    assert_eq!(ledger.balance_of(&worker), 100);
    assert_eq!(ledger.balance_of(&delegator), 400);
}
