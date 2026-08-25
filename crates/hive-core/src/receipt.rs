use crate::error::{HiveError, Result};
use crate::types::{ExecutionDigest, PublicKeyHex, SignatureHex, TaskId};
use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct AgentKeypair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl AgentKeypair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn public_key_hex(&self) -> PublicKeyHex {
        hex::encode(self.verifying_key.to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> SignatureHex {
        let signature = self.signing_key.sign(message);
        hex::encode(signature.to_bytes())
    }

    pub fn sign_message(&self, message: &[u8]) -> SignatureHex {
        self.sign(message)
    }
}

/// Cryptographic Task Receipt produced by a Worker Agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskReceipt {
    pub task_id: TaskId,
    pub worker_id: String,
    pub worker_pubkey: PublicKeyHex,
    pub input_hash: String,
    pub output_hash: String,
    pub output_payload: String,
    pub execution_duration_ms: u64,
    pub timestamp: i64,
    pub execution_digest: ExecutionDigest,
    pub signature: SignatureHex,
}

impl TaskReceipt {
    /// Deterministically computes the input hash, output hash, and compound execution digest
    pub fn compute_digests(
        task_id: TaskId,
        worker_id: &str,
        input_payload: &str,
        output_payload: &str,
        timestamp: i64,
    ) -> (String, String, ExecutionDigest) {
        let mut input_hasher = Sha256::new();
        input_hasher.update(input_payload.as_bytes());
        let input_hash = hex::encode(input_hasher.finalize());

        let mut output_hasher = Sha256::new();
        output_hasher.update(output_payload.as_bytes());
        let output_hash = hex::encode(output_hasher.finalize());

        let mut digest_hasher = Sha256::new();
        let binding_message = format!(
            "{}:{}:{}:{}:{}",
            task_id, worker_id, input_hash, output_hash, timestamp
        );
        digest_hasher.update(binding_message.as_bytes());
        let execution_digest = hex::encode(digest_hasher.finalize());

        (input_hash, output_hash, execution_digest)
    }

    pub fn create_and_sign(
        task_id: TaskId,
        worker_id: impl Into<String>,
        keypair: &AgentKeypair,
        input_payload: &str,
        output_payload: impl Into<String>,
        execution_duration_ms: u64,
    ) -> Self {
        let worker_id_str = worker_id.into();
        let output_str = output_payload.into();
        let timestamp = Utc::now().timestamp();

        let (input_hash, output_hash, execution_digest) = Self::compute_digests(
            task_id,
            &worker_id_str,
            input_payload,
            &output_str,
            timestamp,
        );

        let signature = keypair.sign(execution_digest.as_bytes());

        Self {
            task_id,
            worker_id: worker_id_str,
            worker_pubkey: keypair.public_key_hex(),
            input_hash,
            output_hash,
            output_payload: output_str,
            execution_duration_ms,
            timestamp,
            execution_digest,
            signature,
        }
    }

    /// Verifies that the Ed25519 signature is valid for the execution_digest
    pub fn verify_signature(&self) -> Result<bool> {
        let pubkey_bytes = hex::decode(&self.worker_pubkey)
            .map_err(|e| HiveError::SignatureError(format!("Invalid pubkey hex: {}", e)))?;
        let pubkey_arr: [u8; 32] = pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| HiveError::SignatureError("Public key must be 32 bytes".to_string()))?;

        let verifying_key = VerifyingKey::from_bytes(&pubkey_arr)
            .map_err(|e| HiveError::SignatureError(format!("Invalid verifying key: {}", e)))?;

        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| HiveError::SignatureError(format!("Invalid signature hex: {}", e)))?;
        let sig_arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| HiveError::SignatureError("Signature must be 64 bytes".to_string()))?;

        let signature = Signature::from_bytes(&sig_arr);

        verifying_key
            .verify(self.execution_digest.as_bytes(), &signature)
            .map_err(|e| {
                HiveError::SignatureError(format!("Signature verification failed: {}", e))
            })?;

        Ok(true)
    }

    /// Verifies that the receipt's execution_digest, input_hash, and output_hash
    /// are strictly and cryptographically bound to the actual input and output payloads.
    pub fn verify_content_integrity(&self, expected_input_payload: &str) -> Result<bool> {
        let (expected_input_hash, expected_output_hash, expected_digest) = Self::compute_digests(
            self.task_id,
            &self.worker_id,
            expected_input_payload,
            &self.output_payload,
            self.timestamp,
        );

        if self.input_hash != expected_input_hash {
            return Err(HiveError::TaskExecutionError(format!(
                "Receipt input_hash mismatch: expected {}, got {}",
                expected_input_hash, self.input_hash
            )));
        }

        if self.output_hash != expected_output_hash {
            return Err(HiveError::TaskExecutionError(format!(
                "Receipt output_hash mismatch: expected {}, got {}",
                expected_output_hash, self.output_hash
            )));
        }

        if self.execution_digest != expected_digest {
            return Err(HiveError::TaskExecutionError(format!(
                "Receipt execution_digest mismatch: expected {}, got {}",
                expected_digest, self.execution_digest
            )));
        }

        Ok(true)
    }
}
