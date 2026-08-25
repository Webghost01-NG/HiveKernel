use crate::error::{HiveError, Result};
use crate::receipt::AgentKeypair;
use crate::types::PublicKeyHex;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use ed25519_dalek::SigningKey;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct KeystoreData {
    pub agent_id: String,
    pub public_key: PublicKeyHex,
    pub encrypted: bool,
    pub kdf: String,
    pub salt_hex: Option<String>,
    pub nonce_hex: Option<String>,
    pub ciphertext_hex: String,
}

pub struct KeystoreManager;

impl KeystoreManager {
    /// Saves an Ed25519 keypair encrypted with a passphrase using PBKDF2-HMAC-SHA256 and AES-256-GCM
    pub fn save_encrypted_keypair(
        dir: &Path,
        agent_id: &str,
        keypair: &AgentKeypair,
        passphrase: &str,
    ) -> Result<PathBuf> {
        fs::create_dir_all(dir)
            .map_err(|e| HiveError::AgentError(format!("Failed to create keystore dir: {}", e)))?;

        let file_path = dir.join(format!("{}.key.json", agent_id));
        let secret_bytes = keypair.signing_key.to_bytes();

        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);

        let mut key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.as_bytes(), &salt, 100_000, &mut key);

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| HiveError::AgentError(format!("Cipher initialization failed: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, secret_bytes.as_ref())
            .map_err(|e| HiveError::AgentError(format!("Encryption failed: {}", e)))?;

        let data = KeystoreData {
            agent_id: agent_id.to_string(),
            public_key: keypair.public_key_hex(),
            encrypted: true,
            kdf: "PBKDF2-HMAC-SHA256-100k-AES256GCM".to_string(),
            salt_hex: Some(hex::encode(salt)),
            nonce_hex: Some(hex::encode(nonce_bytes)),
            ciphertext_hex: hex::encode(ciphertext),
        };

        let json = serde_json::to_string_pretty(&data)?;
        let mut file = File::create(&file_path)
            .map_err(|e| HiveError::AgentError(format!("Failed to create key file: {}", e)))?;

        file.write_all(json.as_bytes())
            .map_err(|e| HiveError::AgentError(format!("Failed to write key file: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&file_path)
                .map_err(|e| HiveError::AgentError(e.to_string()))?
                .permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&file_path, perms)
                .map_err(|e| HiveError::AgentError(e.to_string()))?;
        }

        Ok(file_path)
    }

    /// Loads and decrypts an encrypted Ed25519 keypair using the provided passphrase
    pub fn load_encrypted_keypair(file_path: &Path, passphrase: &str) -> Result<AgentKeypair> {
        let mut file = File::open(file_path)
            .map_err(|e| HiveError::AgentError(format!("Failed to open key file: {}", e)))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| HiveError::AgentError(format!("Failed to read key file: {}", e)))?;

        let data: KeystoreData = serde_json::from_str(&content)?;

        if !data.encrypted {
            // Unencrypted fallback
            let secret_bytes = hex::decode(&data.ciphertext_hex)
                .map_err(|e| HiveError::AgentError(format!("Invalid secret key hex: {}", e)))?;
            let secret_arr: [u8; 32] = secret_bytes
                .as_slice()
                .try_into()
                .map_err(|_| HiveError::AgentError("Secret key must be 32 bytes".to_string()))?;
            let signing_key = SigningKey::from_bytes(&secret_arr);
            let verifying_key = signing_key.verifying_key();
            return Ok(AgentKeypair {
                signing_key,
                verifying_key,
            });
        }

        let salt_hex = data.salt_hex.ok_or_else(|| {
            HiveError::AgentError("Missing salt in encrypted keystore".to_string())
        })?;
        let nonce_hex = data.nonce_hex.ok_or_else(|| {
            HiveError::AgentError("Missing nonce in encrypted keystore".to_string())
        })?;

        let salt = hex::decode(&salt_hex)
            .map_err(|e| HiveError::AgentError(format!("Invalid salt hex: {}", e)))?;
        let nonce_bytes = hex::decode(&nonce_hex)
            .map_err(|e| HiveError::AgentError(format!("Invalid nonce hex: {}", e)))?;

        let mut key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.as_bytes(), &salt, 100_000, &mut key);

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| HiveError::AgentError(format!("Cipher initialization failed: {}", e)))?;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = hex::decode(&data.ciphertext_hex)
            .map_err(|e| HiveError::AgentError(format!("Invalid ciphertext hex: {}", e)))?;

        let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).map_err(|_| {
            HiveError::AgentError("Decryption failed: Incorrect passphrase".to_string())
        })?;

        let secret_arr: [u8; 32] = decrypted
            .as_slice()
            .try_into()
            .map_err(|_| HiveError::AgentError("Decrypted key must be 32 bytes".to_string()))?;

        let signing_key = SigningKey::from_bytes(&secret_arr);
        let verifying_key = signing_key.verifying_key();

        Ok(AgentKeypair {
            signing_key,
            verifying_key,
        })
    }

    /// Saves unencrypted keypair with 0600 file permissions
    pub fn save_keypair(dir: &Path, agent_id: &str, keypair: &AgentKeypair) -> Result<PathBuf> {
        Self::save_encrypted_keypair(dir, agent_id, keypair, "default_insecure_passphrase")
    }

    /// Loads unencrypted keypair with default passphrase
    pub fn load_keypair(file_path: &Path) -> Result<AgentKeypair> {
        Self::load_encrypted_keypair(file_path, "default_insecure_passphrase")
    }
}
