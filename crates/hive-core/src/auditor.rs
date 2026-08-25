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

/// Deterministic Static Security & Gas Analysis Engine
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

            // Skip comment-only lines
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }

            // Function boundary reset
            if trimmed.starts_with("function ")
                || trimmed.starts_with("modifier ")
                || trimmed.starts_with("contract ")
            {
                external_call_found = false;
                call_line = 0;
            }

            // 1. Insecure tx.origin authorization
            if line.contains("tx.origin") {
                findings.push(VulnerabilityFinding {
                    severity: "HIGH".to_string(),
                    title: "Insecure Use of tx.origin for Authorization".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "tx.origin is vulnerable to phishing and intermediary proxy/contract attacks.".to_string(),
                    recommendation: "Replace tx.origin with msg.sender for access control checks.".to_string(),
                });
                security_score -= 30;
            }

            // 2. Track external calls
            let has_external_call = line.contains(".call{value:") || line.contains(".call.value(");
            if has_external_call {
                external_call_found = true;
                call_line = line_num;
            }

            // Detect state mutation occurring after the external call (including on the same line if minified)
            let has_mutation = (line.contains("=") || line.contains("-=") || line.contains("+="))
                && !line.contains("==")
                && !line.contains("!=")
                && !line.contains("<=")
                && !line.contains(">=")
                && !line.contains("bytes32")
                && !line.contains("uint256")
                && !line.contains("address")
                && !line.contains("bool");

            if external_call_found {
                // Multi-line reentrancy
                if line_num > call_line && has_mutation {
                    findings.push(VulnerabilityFinding {
                        severity: "CRITICAL".to_string(),
                        title: "Reentrancy Vulnerability (Checks-Effects-Interactions Violation)".to_string(),
                        line_number: line_num,
                        code_snippet: format!(
                            "Call at line {}: {}\nMutation at line {}: {}",
                            call_line,
                            lines.get(call_line - 1).unwrap_or(&"").trim(),
                            line_num,
                            trimmed
                        ),
                        description: "State variable is modified after an external Ether transfer, allowing reentrant drains.".to_string(),
                        recommendation: "Update state variables BEFORE external calls or inherit OpenZeppelin ReentrancyGuard.".to_string(),
                    });
                    security_score -= 45;
                    external_call_found = false;
                }
                // Single-line reentrancy
                else if line_num == call_line {
                    if let Some(call_idx) = line.find(".call") {
                        let after_call = &line[call_idx..];
                        if (after_call.contains("=")
                            || after_call.contains("-=")
                            || after_call.contains("+="))
                            && !after_call.contains("==")
                            && !after_call.contains("!=")
                            && !after_call.contains("bool success")
                            && !after_call.contains("bool s")
                        {
                            findings.push(VulnerabilityFinding {
                                severity: "CRITICAL".to_string(),
                                title: "Reentrancy Vulnerability (Checks-Effects-Interactions Violation)".to_string(),
                                line_number: line_num,
                                code_snippet: trimmed.to_string(),
                                description: "State mutation follows external call on the same statement line.".to_string(),
                                recommendation: "Enforce Checks-Effects-Interactions pattern.".to_string(),
                            });
                            security_score -= 45;
                            external_call_found = false;
                        }
                    }
                }
            }

            // 3. Unchecked low-level call return value
            if line.contains(".call(")
                && !line.contains("require(")
                && !line.contains("(bool success")
                && !line.contains("(bool s")
            {
                findings.push(VulnerabilityFinding {
                    severity: "MEDIUM".to_string(),
                    title: "Unchecked Low-Level Call Return Value".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "Low-level .call() return boolean is not validated, risking silent execution failure.".to_string(),
                    recommendation: "Validate call return: (bool success, ) = target.call(...); require(success);".to_string(),
                });
                security_score -= 15;
            }

            // 4. Deprecated selfdestruct opcode
            if line.contains("selfdestruct(") || line.contains("suicide(") {
                findings.push(VulnerabilityFinding {
                    severity: "HIGH".to_string(),
                    title: "Deprecated selfdestruct Opcode Usage".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "selfdestruct opcode is deprecated under EIP-6780 (Cancun hardfork).".to_string(),
                    recommendation: "Refactor logic to pause or deactivate contracts without opcode destruction.".to_string(),
                });
                security_score -= 20;
            }

            // 5. Unsafe arbitrary delegatecall
            if line.contains(".delegatecall(") && !line.contains("onlyOwner") {
                findings.push(VulnerabilityFinding {
                    severity: "CRITICAL".to_string(),
                    title: "Unrestricted delegatecall Execution".to_string(),
                    line_number: line_num,
                    code_snippet: trimmed.to_string(),
                    description: "Arbitrary delegatecall allows callers to execute malicious bytecode within the contract's storage context.".to_string(),
                    recommendation: "Restrict delegatecall execution targets to an immutable or owner-approved whitelist.".to_string(),
                });
                security_score -= 50;
            }
        }

        // Gas Optimization Grade
        let gas_grade = if code.contains("memory") && !code.contains("calldata") {
            "B- (Use calldata for external function input arguments to reduce gas consumption)"
                .to_string()
        } else if code.contains("uint8") || code.contains("uint16") {
            "B+ (Verify struct packing to prevent 32-byte EVM word padding overhead)".to_string()
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
