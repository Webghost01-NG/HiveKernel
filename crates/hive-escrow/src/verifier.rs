use hive_core::{
    auditor::{AuditReport, ContractAuditor},
    error::{HiveError, Result},
    receipt::TaskReceipt,
    types::TaskSpec,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudProof {
    pub task_id: String,
    pub worker_id: String,
    pub reason: String,
    pub validator_findings_count: usize,
    pub worker_findings_count: usize,
    pub validator_score: u32,
    pub worker_score: u32,
}

/// Multi-Agent Deterministic Optimistic Verifier Node
pub struct SwarmVerifier;

impl SwarmVerifier {
    /// Objective verification: Re-runs the deterministic analysis kernel over the input code
    /// and compares the result mathematically with the worker's signed receipt.
    pub fn verify_work(spec: &TaskSpec, receipt: &TaskReceipt) -> Result<bool> {
        // 1. Verify cryptographic Ed25519 signature
        if !receipt.verify_signature()? {
            return Err(HiveError::SignatureError(
                "Invalid Ed25519 cryptographic signature on TaskReceipt".to_string(),
            ));
        }

        // 2. Verify Task ID integrity
        if receipt.task_id != spec.id {
            return Err(HiveError::TaskExecutionError(
                "Task ID mismatch between specification and receipt".to_string(),
            ));
        }

        // 3. Deserialize worker's output payload into AuditReport
        let worker_report: AuditReport = serde_json::from_str(&receipt.output_payload)
            .map_err(|e| HiveError::TaskExecutionError(format!("Worker returned invalid JSON schema: {}", e)))?;

        // 4. Independent re-execution: Validator audits the exact same code
        let validator_report = ContractAuditor::audit_source(&spec.description, &spec.input_payload);

        // 5. Mathematical consistency check
        if worker_report.total_vulnerabilities != validator_report.total_vulnerabilities {
            let proof = FraudProof {
                task_id: spec.id.to_string(),
                worker_id: receipt.worker_id.clone(),
                reason: format!(
                    "Vulnerability count mismatch: Worker reported {} issues, Validator identified {}",
                    worker_report.total_vulnerabilities, validator_report.total_vulnerabilities
                ),
                validator_findings_count: validator_report.total_vulnerabilities,
                worker_findings_count: worker_report.total_vulnerabilities,
                validator_score: validator_report.security_score,
                worker_score: worker_report.security_score,
            };
            return Err(HiveError::TaskExecutionError(serde_json::to_string(&proof)?));
        }

        if worker_report.security_score != validator_report.security_score {
            let proof = FraudProof {
                task_id: spec.id.to_string(),
                worker_id: receipt.worker_id.clone(),
                reason: format!(
                    "Security score forged: Worker reported {}/100, Validator calculated {}/100",
                    worker_report.security_score, validator_report.security_score
                ),
                validator_findings_count: validator_report.total_vulnerabilities,
                worker_findings_count: worker_report.total_vulnerabilities,
                validator_score: validator_report.security_score,
                worker_score: worker_report.security_score,
            };
            return Err(HiveError::TaskExecutionError(serde_json::to_string(&proof)?));
        }

        Ok(true)
    }
}
