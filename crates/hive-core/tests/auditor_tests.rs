use hive_core::auditor::ContractAuditor;
use hive_core::keystore::KeystoreManager;
use hive_core::receipt::AgentKeypair;
use std::env;

#[test]
fn test_reentrancy_detection_with_line_number() {
    let vulnerable_code = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract VulnerableBank {
    mapping(address => uint256) public balances;

    function withdraw() public {
        uint256 amount = balances[msg.sender];
        (bool success, ) = msg.sender.call{value: amount}("");
        require(success);
        balances[msg.sender] = 0;
    }
}
"#;

    let report = ContractAuditor::audit_source("VulnerableBank.sol", vulnerable_code);
    assert!(report.total_vulnerabilities > 0);
    let reentrancy = report.findings.iter().find(|f| f.title.contains("Reentrancy")).unwrap();
    assert_eq!(reentrancy.severity, "CRITICAL");
    assert_eq!(reentrancy.line_number, 11);
    assert!(report.security_score < 100);
}

#[test]
fn test_clean_contract_audit() {
    let clean_code = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract SafeBank {
    mapping(address => uint256) public balances;

    function deposit(uint256 amount) external {
        balances[msg.sender] += amount;
    }
}
"#;

    let report = ContractAuditor::audit_source("SafeBank.sol", clean_code);
    assert_eq!(report.total_vulnerabilities, 0);
    assert_eq!(report.security_score, 100);
}

#[test]
fn test_keystore_save_and_load() {
    let temp_dir = env::temp_dir().join("hive_keystore_test");
    let keypair = AgentKeypair::generate();
    let original_pubkey = keypair.public_key_hex();

    let path = KeystoreManager::save_keypair(&temp_dir, "test_agent", &keypair).unwrap();
    assert!(path.exists());

    let loaded_keypair = KeystoreManager::load_keypair(&path).unwrap();
    assert_eq!(loaded_keypair.public_key_hex(), original_pubkey);
}
