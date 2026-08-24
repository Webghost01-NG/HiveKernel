use crate::error::{HiveError, Result};
use crate::receipt::AgentKeypair;
use crate::types::PublicKeyHex;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct KeystoreData {
    pub agent_id: String,
    pub public_key: PublicKeyHex,
    pub secret_key_hex: String,
}

pub struct KeystoreManager;

impl KeystoreManager {
    pub fn save_keypair(dir: &Path, agent_id: &str, keypair: &AgentKeypair) -> Result<PathBuf> {
        fs::create_dir_all(dir)
            .map_err(|e| HiveError::AgentError(format!("Failed to create keystore dir: {}", e)))?;

        let file_path = dir.join(format!("{}.key.json", agent_id));
        let secret_bytes = keypair.signing_key.to_bytes();
        let secret_hex = hex::encode(secret_bytes);

        let data = KeystoreData {
            agent_id: agent_id.to_string(),
            public_key: keypair.public_key_hex(),
            secret_key_hex: secret_hex,
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
            perms.set_mode(0o600); // Read/Write only by owner
            fs::set_permissions(&file_path, perms)
                .map_err(|e| HiveError::AgentError(e.to_string()))?;
        }

        Ok(file_path)
    }

    pub fn load_keypair(file_path: &Path) -> Result<AgentKeypair> {
        let mut file = File::open(file_path)
            .map_err(|e| HiveError::AgentError(format!("Failed to open key file: {}", e)))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| HiveError::AgentError(format!("Failed to read key file: {}", e)))?;

        let data: KeystoreData = serde_json::from_str(&content)?;
        let secret_bytes = hex::decode(&data.secret_key_hex)
            .map_err(|e| HiveError::AgentError(format!("Invalid secret key hex: {}", e)))?;

        let secret_arr: [u8; 32] = secret_bytes
            .as_slice()
            .try_into()
            .map_err(|_| HiveError::AgentError("Secret key must be 32 bytes".to_string()))?;

        let signing_key = SigningKey::from_bytes(&secret_arr);
        let verifying_key = signing_key.verifying_key();

        Ok(AgentKeypair {
            signing_key,
            verifying_key,
        })
    }
}
