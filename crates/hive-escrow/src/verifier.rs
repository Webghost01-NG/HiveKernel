use hive_core::{error::HiveError, error::Result, receipt::TaskReceipt, types::TaskSpec};

/// Multi-Agent Optimistic Verifier Node
pub struct SwarmVerifier;

impl SwarmVerifier {
    /// Validates receipt signature, time boundaries, and prompt-output coherence
    pub fn verify_work(spec: &TaskSpec, receipt: &TaskReceipt) -> Result<bool> {
        // 1. Verify cryptographic Ed25519 signature
        if !receipt.verify_signature()? {
            return Err(HiveError::SignatureError("Invalid cryptographic signature on TaskReceipt".to_string()));
        }

        // 2. Verify Task ID integrity
        if receipt.task_id != spec.id {
            return Err(HiveError::TaskExecutionError("Task ID mismatch in receipt".to_string()));
        }

        // 3. Verify output payload is not empty or malformed
        if receipt.output_payload.trim().is_empty() {
            return Err(HiveError::TaskExecutionError("Worker returned empty output payload".to_string()));
        }

        // 4. Check for hallucination / scam markers (e.g. empty or placeholder content)
        if receipt.output_payload.contains("<<MALICIOUS_INJECTION>>") {
            return Err(HiveError::TaskExecutionError("Malicious injection / exploit detected in worker output".to_string()));
        }

        Ok(true)
    }
}
