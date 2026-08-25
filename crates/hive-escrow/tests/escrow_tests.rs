use chrono::{Duration, Utc};
use hive_core::{
    auditor::ContractAuditor,
    receipt::{AgentKeypair, TaskReceipt},
    types::{AgentCapability, TaskSpec, TaskStatus},
};
use hive_escrow::{escrow::EscrowManager, settlement::SwarmLedger, verifier::SwarmVerifier};

#[test]
fn test_escrow_settlement_and_challenge_window() {
    let mut escrow = EscrowManager::new();
    let mut ledger = SwarmLedger::new();
    let keypair = AgentKeypair::generate();

    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0000000000",
        AgentCapability::SmartContractAuditor,
        "Audit Smart Contract",
        "contract A {}",
        100,
    );

    ledger.deposit(&"Delegator_Alpha".to_string(), 500);
    assert!(ledger.withdraw(&"Delegator_Alpha".to_string(), 100));

    escrow
        .lock_escrow(task.id, "Delegator_Alpha".to_string(), 100)
        .unwrap();
    escrow
        .assign_worker(task.id, "Worker_Beta".to_string())
        .unwrap();

    let audit_report = ContractAuditor::audit_source(&task.description, &task.input_payload);
    let output_json = serde_json::to_string(&audit_report).unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Beta",
        &keypair,
        &task.input_payload,
        output_json,
        150,
    );

    escrow.submit_receipt(receipt.clone(), 30).unwrap();
    assert!(SwarmVerifier::verify_work(&task, &receipt).unwrap());

    // 1. Attempting to settle before deadline should fail
    let now = Utc::now();
    assert!(escrow.finalize_settlement_at(task.id, now).is_err());

    // 2. Settling after deadline expires should succeed
    let after_deadline = now + Duration::seconds(35);
    let status = escrow
        .finalize_settlement_at(task.id, after_deadline)
        .unwrap();
    assert_eq!(status, TaskStatus::Settled);

    ledger.deposit(&"Worker_Beta".to_string(), 100);
    assert_eq!(ledger.balance_of(&"Worker_Beta".to_string()), 100);
}

#[test]
fn test_dispute_deadline_enforcement() {
    let mut escrow = EscrowManager::new();
    let keypair = AgentKeypair::generate();

    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0000000000",
        AgentCapability::SmartContractAuditor,
        "Audit Contract",
        "contract A {}",
        100,
    );

    escrow
        .lock_escrow(task.id, "Delegator_Alpha".to_string(), 100)
        .unwrap();
    escrow
        .assign_worker(task.id, "Worker_Beta".to_string())
        .unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Beta",
        &keypair,
        &task.input_payload,
        "{}",
        100,
    );

    escrow.submit_receipt(receipt, 30).unwrap();
    let now = Utc::now();

    // 1. Raising dispute within window succeeds
    assert!(escrow
        .raise_dispute_at(
            task.id,
            "Fraud detected".to_string(),
            now + Duration::seconds(10)
        )
        .is_ok());

    let record = escrow.get_record(&task.id).unwrap();
    assert_eq!(record.status, TaskStatus::Disputed);

    // 2. Finalizing a disputed task slashes the worker
    let status = escrow
        .finalize_settlement_at(task.id, now + Duration::seconds(35))
        .unwrap();
    assert_eq!(status, TaskStatus::Slashed);
}
