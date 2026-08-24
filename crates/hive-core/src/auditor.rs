use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulnerabilityFinding {
    pub severity: String,
    pub title: String,
    pub line_number: usize,
    pub code_snippet: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    pub target_name: String,
    pub total_lines: usize,
    pub total_vulnerabilities: usize,
    pub security_score: u32,
    pub gas_efficiency_grade: String,
    pub findings: Vec<VulnerabilityFinding>,
    pub summary: String,
}

pub struct ContractAuditor;

impl ContractAuditor {
    pub fn audit_source(target_name: &str, code: &str) -> AuditReport {
        let lines: Vec<&str> = code.lines().collect();
        let total_lines = lines.len();
        let mut findings = Vec::new();
        let mut security_score: i32 = 100;

        let mut call_line = 0;
        let mut external_call_found = false;

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            // Skip single-line comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // 1. Check for tx.origin authentication vulnerability
            if line.contains("tx.origin") {
                findings.push(VulnerabilityFinding {
                    severity: "HIGH".to_string(),
                    title: "Insecure Use of tx.origin for Authorization".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "tx.origin is vulnerable to phishing and intermediary contract attacks.".to_string(),
                    recommendation: "Replace tx.origin with msg.sender for all authentication checks.".to_string(),
                });
                security_score -= 30;
            }

            // 2. Track external calls for Reentrancy detection
            if line.contains(".call{value:") || line.contains(".call.value(") {
                external_call_found = true;
                call_line = line_num;
            }

            // Detect state mutation occurring after the external call
            if external_call_found && line_num > call_line {
                if (line.contains("=") || line.contains("-=") || line.contains("+="))
                    && !line.contains("==")
                    && !line.contains("!=")
                    && !line.contains("<=")
                    && !line.contains(">=")
                {
                    findings.push(VulnerabilityFinding {
                        severity: "CRITICAL".to_string(),
                        title: "Reentrancy Vulnerability (Checks-Effects-Interactions Violation)".to_string(),
                        line_number: line_num,
                        code_snippet: format!("Call at line {}: {}\nMutation at line {}: {}", call_line, lines[call_line - 1].trim(), line_num, trimmed),
                        description: "State variable was modified after an external Ether transfer, enabling reentrant drains.".to_string(),
                        recommendation: "Update state variables BEFORE external calls or inherit ReentrancyGuard.".to_string(),
                    });
                    security_score -= 45;
                    external_call_found = false; // Reset to avoid duplicate flags
                }
            }

            // 3. Check for Unchecked Call Return Values
            if line.contains(".call(") && !line.contains("require(") && !line.contains("(bool success") && !line.contains("(bool s") {
                findings.push(VulnerabilityFinding {
                    severity: "MEDIUM".to_string(),
                    title: "Unchecked Low-Level Call Return Value".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "Low-level .call() return value is ignored, risking silent failure.".to_string(),
                    recommendation: "Verify boolean success: (bool success, ) = target.call(...); require(success);".to_string(),
                });
                security_score -= 15;
            }

            // 4. Check for selfdestruct usage
            if line.contains("selfdestruct(") || line.contains("suicide(") {
                findings.push(VulnerabilityFinding {
                    severity: "HIGH".to_string(),
                    title: "Deprecated selfdestruct Opcode Usage".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "selfdestruct behavior is deprecated under Cancun / EIP-6780.".to_string(),
                    recommendation: "Refactor logic to deactivate contract functions instead of destroying bytecode.".to_string(),
                });
                security_score -= 20;
            }

            // 5. Check for unsafe delegatecall
            if line.contains(".delegatecall(") && !line.contains("onlyOwner") {
                findings.push(VulnerabilityFinding {
                    severity: "CRITICAL".to_string(),
                    title: "Unrestricted delegatecall Execution".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "Arbitrary delegatecall allows callers to execute malicious code in the context of this contract.".to_string(),
                    recommendation: "Restrict delegatecall targets to an immutable or owner-approved whitelist.".to_string(),
                });
                security_score -= 50;
            }
        }

        // Gas Optimization Evaluation
        let gas_grade = if code.contains("memory") && !code.contains("calldata") {
            "B- (Use calldata for external function input arguments)".to_string()
        } else if code.contains("uint8") || code.contains("uint16") {
            "B+ (Verify struct packing to prevent non-standard word padding gas overhead)".to_string()
        } else {
            "A+ (Optimized EVM word alignment and storage layout)".to_string()
        };

        let clamped_score = security_score.clamp(0, 100) as u32;
        let summary = if findings.is_empty() {
            format!("Audit Clean for [{}]: 0 vulnerabilities found across {} lines. Security Score: {}/100. Gas Grade: {}", target_name, total_lines, clamped_score, gas_grade)
        } else {
            format!("Audit for [{}]: {} security issue(s) identified across {} lines. Security Score: {}/100.", target_name, findings.len(), total_lines, clamped_score)
        };

        AuditReport {
            target_name: target_name.to_string(),
            total_lines,
            total_vulnerabilities: findings.len(),
            security_score: clamped_score,
            gas_efficiency_grade: gas_grade,
            findings,
            summary,
        }
    }
}
