use base64::Engine;
use rand::rngs::OsRng;
use rand::TryRngCore;
use sha2::{Digest, Sha256};

const SECRET_BYTES: usize = 32;
const TOKEN_PREFIX: &str = "mz_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSecret(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretError {
    NoEntropy,
}

impl SessionSecret {
    pub fn mint() -> Result<Self, SecretError> {
        let mut bytes = [0u8; SECRET_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| SecretError::NoEntropy)?;
        Ok(Self(format!(
            "{TOKEN_PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        )))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> String {
        digest(&self.0)
    }
}

pub fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub fn random_url_safe(bytes: usize) -> Result<String, SecretError> {
    let mut buffer = vec![0u8; bytes];
    OsRng
        .try_fill_bytes(&mut buffer)
        .map_err(|_| SecretError::NoEntropy)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buffer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn a_minted_secret_is_prefixed_and_url_safe() {
        let secret = SessionSecret::mint().unwrap();
        let value = secret.expose();
        assert!(value.starts_with(TOKEN_PREFIX));
        assert!(!value.contains('+'));
        assert!(!value.contains('/'));
        assert!(!value.contains('='));
    }

    #[test]
    fn minting_never_repeats() {
        let set: HashSet<String> = (0..256)
            .map(|_| SessionSecret::mint().unwrap().expose().to_string())
            .collect();
        assert_eq!(set.len(), 256);
    }

    #[test]
    fn the_digest_is_stable_and_sixty_four_hex_characters() {
        let secret = SessionSecret::mint().unwrap();
        assert_eq!(secret.digest(), secret.digest());
        assert_eq!(secret.digest().len(), 64);
        assert!(secret.digest().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn different_secrets_digest_differently() {
        let first = SessionSecret::mint().unwrap();
        let second = SessionSecret::mint().unwrap();
        assert_ne!(first.digest(), second.digest());
    }

    #[test]
    fn the_plaintext_never_appears_in_its_own_digest() {
        let secret = SessionSecret::mint().unwrap();
        assert!(!secret.digest().contains(secret.expose()));
    }

    #[test]
    fn random_url_safe_is_deterministic_in_length() {
        let value = random_url_safe(16).unwrap();
        assert_eq!(value.len(), 22);
    }
}
