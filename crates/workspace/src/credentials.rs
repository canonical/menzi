use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use menzi_common::{MenziError, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpencodeAuth {
    pub username: String,
    pub password: String,
}

#[derive(Clone)]
pub struct WorkspaceCredentialManager {
    key: [u8; 32],
}

impl WorkspaceCredentialManager {
    pub fn from_env() -> Self {
        let secret = std::env::var("MENZI_WORKSPACE_CREDENTIAL_KEY")
            .unwrap_or_else(|_| "menzi-dev-workspace-credential-key".to_string());
        let digest = Sha256::digest(secret.as_bytes());
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest[..32]);
        Self { key }
    }

    pub fn generate_password(&self) -> String {
        Uuid::new_v4().as_simple().to_string()
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce_uuid = Uuid::new_v4();
        let nonce = Nonce::from_slice(&nonce_uuid.as_bytes()[..12]);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|error| MenziError::Internal(anyhow::anyhow!(error.to_string())))?;
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_uuid.as_bytes()[..12]);
        out.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(out))
    }

    pub fn decrypt(&self, payload: &str) -> Result<String> {
        let decoded = BASE64.decode(payload).map_err(|error| {
            MenziError::Validation(format!("invalid credential payload: {error}"))
        })?;
        if decoded.len() < 13 {
            return Err(MenziError::Validation(
                "invalid credential payload length".to_string(),
            ));
        }
        let (nonce, ciphertext) = decoded.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|error| {
                MenziError::Validation(format!("credential decrypt failed: {error}"))
            })?;
        String::from_utf8(plaintext)
            .map_err(|error| MenziError::Validation(format!("credential is not utf8: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_and_decrypt_roundtrip() {
        let manager = WorkspaceCredentialManager::from_env();
        let secret = manager.generate_password();
        let encrypted = manager.encrypt(&secret).unwrap();
        let decrypted = manager.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, secret);
    }
}
