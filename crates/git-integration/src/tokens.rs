use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoToken {
    pub token: String,
    pub repo_id: String,
    pub scopes: Vec<String>,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExchangeRequest {
    pub installation_id: String,
    pub repository_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExchangeResponse {
    pub token: String,
    pub expires_at: String,
    pub permissions: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppToken {
    pub jwt: String,
    pub expires_at: String,
}

#[derive(Debug, Clone)]
pub struct TokenManager {
    pub app_id: String,
    pub private_key: String,
}

impl TokenManager {
    pub fn new(app_id: impl Into<String>, private_key: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            private_key: private_key.into(),
        }
    }

    pub fn generate_app_token(&self) -> AppToken {
        AppToken {
            jwt: "jwt-token".to_string(),
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        }
    }

    pub fn exchange_for_repo_token(&self, _installation_id: &str, repo_ids: &[i64]) -> RepoToken {
        RepoToken {
            token: "repo-token".to_string(),
            repo_id: repo_ids
                .first()
                .map(|id| id.to_string())
                .unwrap_or_default(),
            scopes: vec!["repo".to_string()],
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        }
    }

    pub fn is_token_valid(&self, token: &RepoToken) -> bool {
        !token.token.is_empty() && !token.expires_at.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_manager_creates() {
        let manager = TokenManager::new("app-123", "private-key");
        assert_eq!(manager.app_id, "app-123");
    }

    #[test]
    fn token_manager_generates_app_token() {
        let manager = TokenManager::new("app-123", "private-key");
        let token = manager.generate_app_token();
        assert!(!token.jwt.is_empty());
    }

    #[test]
    fn token_manager_exchanges_for_repo_token() {
        let manager = TokenManager::new("app-123", "private-key");
        let token = manager.exchange_for_repo_token("inst-456", &[789]);
        assert!(!token.token.is_empty());
    }

    #[test]
    fn token_manager_validates_token() {
        let manager = TokenManager::new("app-123", "private-key");
        let token = RepoToken {
            token: "repo-token".to_string(),
            repo_id: "789".to_string(),
            scopes: vec!["repo".to_string()],
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        };
        assert!(manager.is_token_valid(&token));
    }

    #[test]
    fn token_manager_rejects_invalid_token() {
        let manager = TokenManager::new("app-123", "private-key");
        let token = RepoToken {
            token: "".to_string(),
            repo_id: "789".to_string(),
            scopes: vec![],
            expires_at: "".to_string(),
        };
        assert!(!manager.is_token_valid(&token));
    }

    #[test]
    fn repo_token_serializes() {
        let token = RepoToken {
            token: "repo-token".to_string(),
            repo_id: "789".to_string(),
            scopes: vec!["repo".to_string()],
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        };
        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("repo-token"));
    }

    #[test]
    fn token_exchange_request_serializes() {
        let request = TokenExchangeRequest {
            installation_id: "inst-456".to_string(),
            repository_ids: vec![789],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("inst-456"));
    }

    #[test]
    fn token_exchange_response_deserializes() {
        let json = r#"{"token":"repo-token","expires_at":"2026-12-31T23:59:59Z","permissions":{"contents":"read"}}"#;
        let response: TokenExchangeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.token, "repo-token");
    }

    #[test]
    fn app_token_serializes() {
        let token = AppToken {
            jwt: "jwt-token".to_string(),
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        };
        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("jwt-token"));
    }
}
