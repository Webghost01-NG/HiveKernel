use hive_core::{
    auditor::{AuditReport, ContractAuditor},
    receipt::{AgentKeypair, TaskReceipt},
    registry::AgentRegistry,
    types::{AgentCapability, TaskSpec},
};
use hive_escrow::verifier::SwarmVerifier;

#[test]
fn test_verifier_accepts_accurate_audit_work() {
    let keypair = AgentKeypair::generate();
    let code = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Vault {
    mapping(address => uint256) public balances;
    function deposit() external payable { balances[msg.sender] += msg.value; }
}
"#;

    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0011223344",
        AgentCapability::SmartContractAuditor,
        "Audit Clean Vault",
        code,
        100,
    );

    let mut registry = AgentRegistry::new();
    registry.register_agent("Worker_Auditor", keypair.public_key_hex());

    let audit_report = ContractAuditor::audit_source(&task.description, &task.input_payload);
    let output_json = serde_json::to_string(&audit_report).unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Auditor",
        &keypair,
        &task.input_payload,
        output_json,
        200,
    );

    assert!(SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry)).unwrap());
}

#[test]
fn test_verifier_catches_forged_clean_report_on_vulnerable_code() {
    let keypair = AgentKeypair::generate();
    let vulnerable_code = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract ExploitVault {
    mapping(address => uint256) public balances;
    function withdraw() external {
        (bool s, ) = msg.sender.call{value: balances[msg.sender]}("");
        require(s);
        balances[msg.sender] = 0; // Reentrancy
    }
}
"#;

    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0011223344",
        AgentCapability::SmartContractAuditor,
        "Audit Vulnerable Vault",
        vulnerable_code,
        150,
    );

    let mut registry = AgentRegistry::new();
    registry.register_agent("Malicious_Worker", keypair.public_key_hex());

    // Malicious worker attempts to forge a clean report with 0 vulnerabilities
    let forged_report = AuditReport {
        target_name: task.description.clone(),
        total_lines: 10,
        total_vulnerabilities: 0,
        security_score: 100,
        gas_efficiency_grade: "A+".to_string(),
        findings: vec![],
        summary: "Forged clean audit report".to_string(),
    };
    let forged_json = serde_json::to_string(&forged_report).unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Malicious_Worker",
        &keypair,
        &task.input_payload,
        forged_json,
        150,
    );

    let verification = SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry));
    assert!(verification.is_err());
    let err_msg = verification.unwrap_err().to_string();
    assert!(err_msg.contains("Vulnerability count mismatch"));
}

#[test]
fn test_verifier_rejects_identity_spoofing() {
    let genuine_keypair = AgentKeypair::generate();
    let attacker_keypair = AgentKeypair::generate();

    let task = TaskSpec::new(
        "Delegator_Alpha",
        "0011223344",
        AgentCapability::SmartContractAuditor,
        "Audit Task",
        "contract A {}",
        100,
    );

    let mut registry = AgentRegistry::new();
    registry.register_agent("Worker_Beta", genuine_keypair.public_key_hex());

    let audit_report = ContractAuditor::audit_source(&task.description, &task.input_payload);
    let output_json = serde_json::to_string(&audit_report).unwrap();

    // Attacker signs using attacker_keypair but claims to be Worker_Beta
    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Beta",
        &attacker_keypair,
        &task.input_payload,
        output_json,
        100,
    );

    let result = SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Public key spoofing detected"));
}
