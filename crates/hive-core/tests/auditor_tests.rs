use hive_core::auditor::ContractAuditor;

#[test]
fn test_reentrancy_detection() {
    let vulnerable_code = r#"
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

    let report = ContractAuditor::audit_solidity_code(vulnerable_code);
    assert!(report.total_vulnerabilities > 0);
    assert!(report.findings.iter().any(|f| f.title.contains("Reentrancy")));
    assert!(report.security_score < 100);
}

#[test]
fn test_clean_contract_audit() {
    let clean_code = r#"
    contract SafeBank {
        mapping(address => uint256) public balances;
        function deposit(uint256 amount) external {
            balances[msg.sender] += amount;
        }
    }
    "#;

    let report = ContractAuditor::audit_solidity_code(clean_code);
    assert_eq!(report.total_vulnerabilities, 0);
    assert_eq!(report.security_score, 100);
}
