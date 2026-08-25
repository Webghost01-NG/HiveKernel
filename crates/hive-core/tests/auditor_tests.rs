use hive_core::auditor::ContractAuditor;
use hive_core::keystore::KeystoreManager;
use hive_core::receipt::AgentKeypair;
use hive_core::registry::AgentRegistry;
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
    let reentrancy = report
        .findings
        .iter()
        .find(|f| f.title.contains("Reentrancy"))
        .unwrap();
    assert_eq!(reentrancy.severity, "CRITICAL");
    assert_eq!(reentrancy.line_number, 11);
    assert!(report.security_score < 100);
}

#[test]
fn test_single_line_reentrancy_detection() {
    let single_line_code = "contract Vault { mapping(address => uint) b; function w() public { (bool s,) = msg.sender.call{value: 1}(\"\"); b[msg.sender] = 0; } }";
    let report = ContractAuditor::audit_source("Vault.sol", single_line_code);
    assert!(report.total_vulnerabilities > 0);
    assert!(report
        .findings
        .iter()
        .any(|f| f.title.contains("Reentrancy")));
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
fn test_keystore_encrypted_save_and_load() {
    let temp_dir = env::temp_dir().join("hive_keystore_enc_test");
    let keypair = AgentKeypair::generate();
    let original_pubkey = keypair.public_key_hex();
    let passphrase = "super_secret_agent_password_123";

    let path =
        KeystoreManager::save_encrypted_keypair(&temp_dir, "test_agent_enc", &keypair, passphrase)
            .unwrap();
    assert!(path.exists());

    let loaded_keypair = KeystoreManager::load_encrypted_keypair(&path, passphrase).unwrap();
    assert_eq!(loaded_keypair.public_key_hex(), original_pubkey);

    // Bad passphrase should fail decryption
    let bad_load = KeystoreManager::load_encrypted_keypair(&path, "wrong_password");
    assert!(bad_load.is_err());
}

#[test]
fn test_agent_registry_identity_binding() {
    let mut registry = AgentRegistry::new();
    let keypair_a = AgentKeypair::generate();
    let keypair_b = AgentKeypair::generate();

    registry.register_agent("Worker_Beta", keypair_a.public_key_hex());

    assert!(registry
        .verify_agent_identity("Worker_Beta", &keypair_a.public_key_hex())
        .is_ok());
    assert!(registry
        .verify_agent_identity("Worker_Beta", &keypair_b.public_key_hex())
        .is_err());
    assert!(registry
        .verify_agent_identity("Unknown_Agent", &keypair_a.public_key_hex())
        .is_err());
}
