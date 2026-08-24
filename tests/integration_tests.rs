use hive_core::{
    agent::AgentProfile,
    receipt::{AgentKeypair, TaskReceipt},
    types::{AgentCapability, TaskSpec, TaskStatus},
};
use hive_escrow::{
    escrow::EscrowManager,
    settlement::SwarmLedger,
    verifier::SwarmVerifier,
};
use hive_p2p::{
    auction::{AuctionMatcher, CandidateBid},
    network::SwarmMeshRouter,
    protocol::SwarmMessage,
};

#[tokio::test]
async fn test_full_optimistic_lifecycle_success() {
    let mut ledger = SwarmLedger::new();
    let mut escrow = EscrowManager::new();
    let router = SwarmMeshRouter::new(10);

    let delegator = "Agent_Delegator".to_string();
    let worker = "Agent_Worker_Beta".to_string();
    let worker_key = AgentKeypair::generate();

    ledger.deposit(&delegator, 500);
    assert_eq!(ledger.balance_of(&delegator), 500);

    let task = TaskSpec::new(
        delegator.clone(),
        "0011223344".to_string(),
        AgentCapability::SmartContractAuditor,
        "Audit Staking Contract",
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

    // 3. Worker executes & signs
    let receipt = TaskReceipt::create_and_sign(
        task.id,
        worker.clone(),
        &worker_key,
        &task.input_payload,
        "Audit passed: 0 vulnerabilities found.",
        250,
    );

    assert!(escrow.submit_receipt(receipt.clone(), 10).is_ok());

    // 4. Validator verifies
    let is_valid = SwarmVerifier::verify_work(&task, &receipt).unwrap();
    assert!(is_valid);

    // 5. Finalize settlement
    let final_status = escrow.finalize_settlement(task.id).unwrap();
    assert_eq!(final_status, TaskStatus::Settled);

    ledger.deposit(&worker, 100);
    assert_eq!(ledger.balance_of(&worker), 100);
    assert_eq!(ledger.balance_of(&delegator), 400);
}

#[tokio::test]
async fn test_dispute_and_slashing_on_fraud() {
    let mut ledger = SwarmLedger::new();
    let mut escrow = EscrowManager::new();

    let delegator = "Agent_Delegator".to_string();
    let worker = "Malicious_Worker".to_string();
    let worker_key = AgentKeypair::generate();

    ledger.deposit(&delegator, 500);
    let task = TaskSpec::new(
        delegator.clone(),
        "0011223344".to_string(),
        AgentCapability::SmartContractAuditor,
        "Audit Oracle Hook",
        "contract Oracle {}",
        200,
    );

    ledger.withdraw(&delegator, 200);
    escrow.lock_escrow(task.id, delegator.clone(), 200).unwrap();
    escrow.assign_worker(task.id, worker.clone()).unwrap();

    // Fraudulent receipt with malicious payload
    let receipt = TaskReceipt::create_and_sign(
        task.id,
        worker.clone(),
        &worker_key,
        &task.input_payload,
        "Exploit injection: <<MALICIOUS_INJECTION>> bypass state",
        100,
    );

    escrow.submit_receipt(receipt.clone(), 10).unwrap();

    // Verification fails
    let verification_result = SwarmVerifier::verify_work(&task, &receipt);
    assert!(verification_result.is_err());

    // Raise dispute
    escrow.raise_dispute(task.id, "Malicious payload detected".to_string()).unwrap();
    let status = escrow.finalize_settlement(task.id).unwrap();
    assert_eq!(status, TaskStatus::Slashed);

    // Refund delegator
    ledger.deposit(&delegator, 200);
    assert_eq!(ledger.balance_of(&delegator), 500);
    assert_eq!(ledger.balance_of(&worker), 0);
}
