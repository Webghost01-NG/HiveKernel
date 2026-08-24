use hive_core::*;

#[test]
fn test_keypair_generation_and_signing() {
    let keypair = AgentKeypair::generate();
    let pubkey_hex = keypair.public_key_hex();
    assert_eq!(pubkey_hex.len(), 64);

    let message = b"Swarm Village: Decentralized AI multi-agent task";
    let sig_hex = keypair.sign_message(message);
    assert_eq!(sig_hex.len(), 128);
}

#[test]
fn test_task_receipt_cryptographic_verification() {
    let worker_keypair = AgentKeypair::generate();
    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0000000000000000000000000000000000000000000000000000000000000000",
        AgentCapability::SmartContractAuditor,
        "Audit Uniswap V4 Hook for Reentrancy",
        "pragma solidity ^0.8.20; contract Hook { ... }",
        150,
    );

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Auditor_Beta",
        &worker_keypair,
        &task.input_payload,
        "Audit Complete: 0 vulnerabilities found, gas optimization score 98/100",
        450,
    );

    assert!(receipt.verify_signature().unwrap());
}
