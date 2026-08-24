use hive_core::{
    auditor::{AuditReport, ContractAuditor},
    receipt::{AgentKeypair, TaskReceipt},
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

    assert!(SwarmVerifier::verify_work(&task, &receipt).unwrap());
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

    // Malicious worker forges a clean report with 0 vulnerabilities
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

    let verification = SwarmVerifier::verify_work(&task, &receipt);
    assert!(verification.is_err());
    let err_msg = verification.unwrap_err().to_string();
    assert!(err_msg.contains("Vulnerability count mismatch"));
}
