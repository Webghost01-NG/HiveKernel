use hive_core::{
    receipt::{AgentKeypair, TaskReceipt},
    types::{AgentCapability, TaskSpec, TaskStatus},
};
use hive_escrow::{
    escrow::EscrowManager,
    settlement::SwarmLedger,
    verifier::SwarmVerifier,
};

#[test]
fn test_escrow_settlement_flow() {
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

    escrow.lock_escrow(task.id, "Delegator_Alpha".to_string(), 100).unwrap();
    escrow.assign_worker(task.id, "Worker_Beta".to_string()).unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Beta",
        &keypair,
        &task.input_payload,
        "Clean report: 0 issues.",
        150,
    );

    escrow.submit_receipt(receipt.clone(), 30).unwrap();
    assert!(SwarmVerifier::verify_work(&task, &receipt).unwrap());

    let status = escrow.finalize_settlement(task.id).unwrap();
    assert_eq!(status, TaskStatus::Settled);

    ledger.deposit(&"Worker_Beta".to_string(), 100);
    assert_eq!(ledger.balance_of(&"Worker_Beta".to_string()), 100);
}
