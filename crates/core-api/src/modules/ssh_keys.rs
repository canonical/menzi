use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::fs;
use std::process::Command;
use uuid::Uuid;

use crate::identity::{caller_uuid, Caller};

#[derive(Serialize)]
pub struct UserSshKeyResponse {
    pub id: String,
    pub label: String,
    pub public_key: String,
    pub fingerprint_sha256: String,
    pub is_default: bool,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateUserSshKeyRequest {
    pub label: String,
    pub public_key: String,
    pub private_key: String,
    pub passphrase: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateUserSshKeyRequest {
    pub label: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Serialize)]
pub struct GeneratedUserSshKeyResponse {
    pub key: UserSshKeyResponse,
    pub private_key: String,
    pub public_key: String,
}

#[derive(Deserialize)]
pub struct GenerateUserSshKeyRequest {
    pub label: String,
    pub is_default: Option<bool>,
}

type SshKeyRow = (
    String,
    String,
    String,
    String,
    bool,
    Option<String>,
    String,
    String,
);

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn map_row(row: SshKeyRow) -> UserSshKeyResponse {
    UserSshKeyResponse {
        id: row.0,
        label: row.1,
        public_key: row.2,
        fingerprint_sha256: row.3,
        is_default: row.4,
        last_used_at: row.5,
        created_at: row.6,
        updated_at: row.7,
    }
}

fn validate_public_key(value: &str) -> bool {
    let trimmed = value.trim();
    let mut parts = trimmed.split_whitespace();
    let kind = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default();
    if kind.is_empty() || body.is_empty() {
        return false;
    }
    if !(kind.starts_with("ssh-") || kind.starts_with("ecdsa-") || kind.starts_with("sk-")) {
        return false;
    }
    body
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=')
}

fn fingerprint_of(public_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key.trim().as_bytes());
    format!("SHA256:{:x}", hasher.finalize())
}

fn generate_keypair(comment: &str) -> Result<(String, String), String> {
    let base = std::path::Path::new("/tmp/opencode");
    let path = base.join(format!("menzi_ssh_{}_ed25519", Uuid::new_v4().as_simple()));
    let status = Command::new("ssh-keygen")
        .arg("-q")
        .arg("-t")
        .arg("ed25519")
        .arg("-N")
        .arg("")
        .arg("-C")
        .arg(comment)
        .arg("-f")
        .arg(path.as_os_str())
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("pub"));
        return Err("ssh-keygen failed".to_string());
    }
    let private_key = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let public_key = fs::read_to_string(path.with_extension("pub")).map_err(|error| error.to_string())?;
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_extension("pub"));
    Ok((private_key, public_key.trim().to_string()))
}

#[derive(Clone)]
struct SshKeyCipher {
    key: [u8; 32],
}

impl SshKeyCipher {
    fn from_env() -> Self {
        let secret = std::env::var("MENZI_SSH_KEY_ENCRYPTION_KEY")
            .unwrap_or_else(|_| "menzi-dev-ssh-key-encryption-key".to_string());
        let digest = Sha256::digest(secret.as_bytes());
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest[..32]);
        Self { key }
    }

    fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce_uuid = Uuid::new_v4();
        let nonce = Nonce::from_slice(&nonce_uuid.as_bytes()[..12]);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|error| error.to_string())?;
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_uuid.as_bytes()[..12]);
        out.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(out))
    }
}

pub async fn list_user_ssh_keys(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let rows = sqlx::query_as::<_, SshKeyRow>(
        "SELECT id::text, label, public_key, fingerprint_sha256, is_default, last_used_at::text, created_at::text, updated_at::text
         FROM user_ssh_keys
         WHERE user_id = $1::uuid
         ORDER BY is_default DESC, created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await;
    match rows {
        Ok(rows) => Json(rows.into_iter().map(map_row).collect::<Vec<_>>()).into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn create_user_ssh_key(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Json(body): Json<CreateUserSshKeyRequest>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    if body.label.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "label is required");
    }
    if body.private_key.trim().is_empty() || !validate_public_key(&body.public_key) {
        return error_response(StatusCode::BAD_REQUEST, "invalid SSH key data");
    }
    let cipher = SshKeyCipher::from_env();
    let encrypted_private = match cipher.encrypt(body.private_key.as_str()) {
        Ok(value) => value,
        Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    let encrypted_passphrase = match body.passphrase.as_deref() {
        Some(value) if !value.is_empty() => match cipher.encrypt(value) {
            Ok(value) => Some(value),
            Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
        },
        _ => None,
    };
    if body.is_default.unwrap_or(false) {
        let _ = sqlx::query("UPDATE user_ssh_keys SET is_default = false WHERE user_id = $1::uuid")
            .bind(user_id)
            .execute(&pool)
            .await;
    }
    let row = sqlx::query_as::<_, SshKeyRow>(
        "INSERT INTO user_ssh_keys
            (user_id, label, public_key, fingerprint_sha256, private_key_encrypted, passphrase_encrypted, is_default)
         VALUES ($1::uuid, $2, $3, $4, $5, $6, $7)
         RETURNING id::text, label, public_key, fingerprint_sha256, is_default, last_used_at::text, created_at::text, updated_at::text",
    )
    .bind(user_id)
    .bind(body.label.trim())
    .bind(body.public_key.trim())
    .bind(fingerprint_of(&body.public_key))
    .bind(encrypted_private)
    .bind(encrypted_passphrase)
    .bind(body.is_default.unwrap_or(false))
    .fetch_one(&pool)
    .await;
    match row {
        Ok(row) => (StatusCode::CREATED, Json(map_row(row))).into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn generate_user_ssh_key(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Json(body): Json<GenerateUserSshKeyRequest>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    if body.label.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "label is required");
    }
    let (private_key, public_key) = match generate_keypair(&format!("menzi@{user_id}")) {
        Ok(pair) => pair,
        Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    let cipher = SshKeyCipher::from_env();
    let encrypted_private = match cipher.encrypt(private_key.as_str()) {
        Ok(value) => value,
        Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    if body.is_default.unwrap_or(false) {
        let _ = sqlx::query("UPDATE user_ssh_keys SET is_default = false WHERE user_id = $1::uuid")
            .bind(user_id)
            .execute(&pool)
            .await;
    }
    let row = sqlx::query_as::<_, SshKeyRow>(
        "INSERT INTO user_ssh_keys
            (user_id, label, public_key, fingerprint_sha256, private_key_encrypted, is_default)
         VALUES ($1::uuid, $2, $3, $4, $5, $6)
         RETURNING id::text, label, public_key, fingerprint_sha256, is_default, last_used_at::text, created_at::text, updated_at::text",
    )
    .bind(user_id)
    .bind(body.label.trim())
    .bind(public_key.trim())
    .bind(fingerprint_of(&public_key))
    .bind(encrypted_private)
    .bind(body.is_default.unwrap_or(false))
    .fetch_one(&pool)
    .await;
    match row {
        Ok(row) => {
            let key = map_row(row);
            (
                StatusCode::CREATED,
                Json(GeneratedUserSshKeyResponse {
                    key,
                    private_key,
                    public_key,
                }),
            )
                .into_response()
        }
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn update_user_ssh_key(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserSshKeyRequest>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let Ok(id) = Uuid::parse_str(&id) else {
        return error_response(StatusCode::NOT_FOUND, "ssh key not found");
    };
    if body.is_default == Some(true) {
        let _ = sqlx::query("UPDATE user_ssh_keys SET is_default = false WHERE user_id = $1::uuid")
            .bind(user_id)
            .execute(&pool)
            .await;
    }
    let row = sqlx::query_as::<_, SshKeyRow>(
        "UPDATE user_ssh_keys
         SET label = COALESCE($3, label),
             is_default = COALESCE($4, is_default),
             updated_at = now()
         WHERE id = $1::uuid AND user_id = $2::uuid
         RETURNING id::text, label, public_key, fingerprint_sha256, is_default, last_used_at::text, created_at::text, updated_at::text",
    )
    .bind(id)
    .bind(user_id)
    .bind(body.label.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(body.is_default)
    .fetch_optional(&pool)
    .await;
    match row {
        Ok(Some(row)) => Json(map_row(row)).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "ssh key not found"),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn delete_user_ssh_key(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<String>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let Ok(id) = Uuid::parse_str(&id) else {
        return error_response(StatusCode::NOT_FOUND, "ssh key not found");
    };
    let result = sqlx::query("DELETE FROM user_ssh_keys WHERE id = $1::uuid AND user_id = $2::uuid")
        .bind(id)
        .bind(user_id)
        .execute(&pool)
        .await;
    match result {
        Ok(result) if result.rows_affected() > 0 => {
            (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Ok(_) => error_response(StatusCode::NOT_FOUND, "ssh key not found"),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{fingerprint_of, generate_keypair, validate_public_key, SshKeyCipher};

    #[test]
    fn validates_public_keys() {
        assert!(validate_public_key("ssh-ed25519 AAAAC3Nza menzi@test"));
        assert!(!validate_public_key("nope"));
    }

    #[test]
    fn fingerprints_are_stable() {
        let key = "ssh-ed25519 AAAAC3Nza menzi@test";
        assert_eq!(fingerprint_of(key), fingerprint_of(key));
    }

    #[test]
    fn cipher_encrypts_data() {
        let cipher = SshKeyCipher::from_env();
        let encrypted = cipher.encrypt("secret").unwrap();
        assert_ne!(encrypted, "secret");
    }

    #[test]
    fn generated_keypair_has_openssh_public_key() {
        let (_, public) = generate_keypair("menzi@test").unwrap();
        assert!(validate_public_key(&public));
        assert!(public.starts_with("ssh-ed25519 "));
    }
}
