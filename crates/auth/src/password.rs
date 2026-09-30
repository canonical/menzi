use argon2::password_hash::phc::PasswordHash as Argon2Hash;

const ARGON2ID: &str = "argon2id";
use argon2::{Argon2, PasswordHasher, PasswordVerifier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordError {
    Hashing,
    Malformed,
}

impl PasswordHash {
    pub fn hash(plaintext: &str) -> Result<Self, PasswordError> {
        Argon2::default()
            .hash_password(plaintext.as_bytes())
            .map(|hash| Self(hash.to_string()))
            .map_err(|_| PasswordError::Hashing)
    }

    pub fn verify(&self, plaintext: &str) -> bool {
        let parsed = match Argon2Hash::new(&self.0) {
            Ok(parsed) => parsed,
            Err(_) => return false,
        };
        Argon2::default()
            .verify_password(plaintext.as_bytes(), &parsed)
            .is_ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn needs_rehash(&self) -> bool {
        match Argon2Hash::new(&self.0) {
            Ok(parsed) => parsed.algorithm.as_str() != ARGON2ID,
            Err(_) => true,
        }
    }
}

pub fn verify_stored(stored: &str, candidate: &str) -> bool {
    let parsed = match Argon2Hash::new(stored) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(candidate.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hash_verifies_its_own_password() {
        let hash = PasswordHash::hash("correct horse battery staple").unwrap();
        assert!(hash.verify("correct horse battery staple"));
    }

    #[test]
    fn a_hash_refuses_a_different_password() {
        let hash = PasswordHash::hash("correct horse battery staple").unwrap();
        assert!(!hash.verify("nope"));
        assert!(!hash.verify(""));
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        let first = PasswordHash::hash("repeated").unwrap();
        let second = PasswordHash::hash("repeated").unwrap();
        assert_ne!(first, second);
        assert!(first.verify("repeated"));
        assert!(second.verify("repeated"));
    }

    #[test]
    fn a_stored_hash_is_argon2id() {
        let hash = PasswordHash::hash("pw").unwrap();
        assert!(hash.as_str().starts_with("$argon2id$"));
    }

    #[test]
    fn a_malformed_stored_hash_denies_rather_than_failing() {
        assert!(!verify_stored("not-a-hash", "pw"));
        assert!(!verify_stored("", "pw"));
        let corrupt = PasswordHash("garbage".to_string());
        assert!(!corrupt.verify("pw"));
    }

    #[test]
    fn explicit_parameters_are_recorded_and_still_verify() {
        let params = argon2::Params::new(19 * 1024, 2, 1, Some(32)).unwrap();
        let algorithm = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let stored = algorithm.hash_password(b"pw").unwrap().to_string();
        assert!(stored.contains("m=19456"));
        assert!(verify_stored(&stored, "pw"));
        assert!(!verify_stored(&stored, "other"));
    }

    #[test]
    fn a_default_hash_does_not_need_rehashing() {
        let hash = PasswordHash::hash("pw").unwrap();
        assert!(!hash.needs_rehash());
    }
}
