use hive_core::{
    auditor::{AuditReport, ContractAuditor},
    error::{HiveError, Result},
    receipt::TaskReceipt,
    registry::AgentRegistry,
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
    /// Objective verification:
    /// 1. Verifies Ed25519 signature validity.
    /// 2. Verifies cryptographic content binding (TaskID, WorkerID, InputHash, OutputHash, Timestamp -> ExecutionDigest).
    /// 3. Validates Worker identity against Swarm Registry if provided.
    /// 4. Independently re-executes the deterministic analysis kernel over the input code.
    /// 5. Compares vulnerability count and security scores mathematically.
    pub fn verify_work_with_registry(
        spec: &TaskSpec,
        receipt: &TaskReceipt,
        registry: Option<&AgentRegistry>,
    ) -> Result<bool> {
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

        // 3. Verify cryptographic binding to actual input and output payloads
        receipt.verify_content_integrity(&spec.input_payload)?;

        // 4. Verify Worker identity in AgentRegistry if available
        if let Some(reg) = registry {
            reg.verify_agent_identity(&receipt.worker_id, &receipt.worker_pubkey)?;
        }

        // 5. Deserialize worker's output payload into AuditReport
        let worker_report: AuditReport =
            serde_json::from_str(&receipt.output_payload).map_err(|e| {
                HiveError::TaskExecutionError(format!("Worker returned invalid JSON schema: {}", e))
            })?;

        // 6. Independent re-execution: Validator audits the exact same code
        let validator_report =
            ContractAuditor::audit_source(&spec.description, &spec.input_payload);

        // 7. Mathematical consistency checks
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
            return Err(HiveError::TaskExecutionError(serde_json::to_string(
                &proof,
            )?));
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
            return Err(HiveError::TaskExecutionError(serde_json::to_string(
                &proof,
            )?));
        }

        Ok(true)
    }

    pub fn verify_work(spec: &TaskSpec, receipt: &TaskReceipt) -> Result<bool> {
        Self::verify_work_with_registry(spec, receipt, None)
    }
}
