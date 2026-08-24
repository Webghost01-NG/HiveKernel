use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityFinding {
    pub severity: String,
    pub title: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub total_vulnerabilities: usize,
    pub security_score: u32,
    pub gas_efficiency_grade: String,
    pub findings: Vec<VulnerabilityFinding>,
    pub summary: String,
}

pub struct ContractAuditor;

impl ContractAuditor {
    pub fn audit_solidity_code(code: &str) -> AuditReport {
        let mut findings = Vec::new();
        let mut security_score: i32 = 100;

        // 1. Check for tx.origin authentication vulnerability
        if code.contains("tx.origin") {
            findings.push(VulnerabilityFinding {
                severity: "HIGH".to_string(),
                title: "Insecure Use of tx.origin for Authentication".to_string(),
                description: "tx.origin was detected in contract logic. Phishing attacks can bypass authorization.".to_string(),
                recommendation: "Replace tx.origin with msg.sender for authorization checks.".to_string(),
            });
            security_score -= 30;
        }

        // 2. Check for Potential Reentrancy pattern (low-level .call without state check)
        if code.contains(".call{value:") || code.contains(".call.value(") {
            let lines: Vec<&str> = code.lines().collect();
            let mut call_found = false;
            let mut write_after_call = false;

            for line in lines {
                if line.contains(".call{value:") || line.contains(".call.value(") {
                    call_found = true;
                } else if call_found && (line.contains("=") || line.contains("-=") || line.contains("+=")) && !line.contains("==") {
                    write_after_call = true;
                }
            }

            if write_after_call {
                findings.push(VulnerabilityFinding {
                    severity: "CRITICAL".to_string(),
                    title: "Reentrancy Vulnerability (Checks-Effects-Interactions Violation)".to_string(),
                    description: "State variable modification detected after external Ether transfer call.".to_string(),
                    recommendation: "Apply Checks-Effects-Interactions pattern or inherit OpenZeppelin ReentrancyGuard.".to_string(),
                });
                security_score -= 45;
            }
        }

        // 3. Check for Unchecked Call Return Values
        if code.contains(".call(") && !code.contains("require(") && !code.contains("(bool success") {
            findings.push(VulnerabilityFinding {
                severity: "MEDIUM".to_string(),
                title: "Unchecked External Low-Level Call Return Value".to_string(),
                description: "Low-level .call() return boolean was not explicitly verified.".to_string(),
                recommendation: "Always check the success boolean returned by external calls: require(success, 'call failed')".to_string(),
            });
            security_score -= 15;
        }

        // 4. Check for selfdestruct usage
        if code.contains("selfdestruct(") || code.contains("suicide(") {
            findings.push(VulnerabilityFinding {
                severity: "HIGH".to_string(),
                title: "Deprecated selfdestruct Opcode Usage".to_string(),
                description: "selfdestruct behavior is altered and deprecated in modern EVM hard forks (Cancun / EIP-6780).".to_string(),
                recommendation: "Refactor logic to avoid reliance on contract self-destruction.".to_string(),
            });
            security_score -= 20;
        }

        // 5. Gas optimization check
        let gas_grade = if code.contains("memory") && !code.contains("calldata") {
            "B- (Consider using calldata for external read-only params)".to_string()
        } else {
            "A+ (Optimized storage access)".to_string()
        };

        let clamped_score = security_score.clamp(0, 100) as u32;
        let summary = if findings.is_empty() {
            format!("Static Analysis Clean: 0 vulnerabilities found. Security Score: {}/100. Gas Grade: {}", clamped_score, gas_grade)
        } else {
            format!("Vulnerabilities Identified: {} issues found. Security Score: {}/100. Action required.", findings.len(), clamped_score)
        };

        AuditReport {
            total_vulnerabilities: findings.len(),
            security_score: clamped_score,
            gas_efficiency_grade: gas_grade,
            findings,
            summary,
        }
    }
}
