use hive_core::{
    receipt::{AgentKeypair, TaskReceipt},
    types::{AgentCapability, TaskSpec},
};

#[test]
fn test_keypair_generation_and_signing() {
    let keypair = AgentKeypair::generate();
    let message = b"HiveKernel_Swarm_Village_Task_Payload";
    let sig_hex = keypair.sign_message(message);

    assert_eq!(keypair.public_key_hex().len(), 64);
    assert_eq!(sig_hex.len(), 128);
}

#[test]
fn test_task_receipt_cryptographic_verification() {
    let keypair = AgentKeypair::generate();
    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0011223344",
        AgentCapability::SmartContractAuditor,
        "Audit Vault.sol",
        "contract Vault {}",
        100,
    );

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Beta",
        &keypair,
        &task.input_payload,
        "Audit Complete: 0 vulnerabilities found, gas optimization score 98/100",
        250,
    );

    assert!(receipt.verify_signature().unwrap());
    assert!(receipt
        .verify_content_integrity(&task.input_payload)
        .unwrap());

    // Tampered payload must fail content integrity verification
    assert!(receipt.verify_content_integrity("tampered input").is_err());
}
