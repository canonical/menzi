use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowRequest {
    pub client_id: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i32,
    pub interval: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i32,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStore {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: String,
    pub server_url: String,
}

impl TokenStore {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            access_token: String::new(),
            refresh_token: None,
            expires_at: String::new(),
            server_url: server_url.into(),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        !self.access_token.is_empty()
    }

    pub fn save_token(&mut self, token: TokenResponse) {
        self.access_token = token.access_token;
        self.refresh_token = token.refresh_token;
        self.expires_at = "2026-12-31T23:59:59Z".to_string();
    }

    pub fn clear(&mut self) {
        self.access_token.clear();
        self.refresh_token = None;
        self.expires_at.clear();
    }
}

#[derive(Debug, Clone)]
pub struct AuthFlow {
    pub client_id: String,
    pub server_url: String,
}

impl AuthFlow {
    pub fn new(client_id: impl Into<String>, server_url: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            server_url: server_url.into(),
        }
    }

    pub fn start_device_flow(&self) -> DeviceFlowResponse {
        DeviceFlowResponse {
            device_code: "device-code".to_string(),
            user_code: "user-code".to_string(),
            verification_uri: format!("{}/device", self.server_url),
            expires_in: 900,
            interval: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_flow_creates() {
        let flow = AuthFlow::new("client-123", "https://menzi.example.com");
        assert_eq!(flow.client_id, "client-123");
    }

    #[test]
    fn auth_flow_starts_device_flow() {
        let flow = AuthFlow::new("client-123", "https://menzi.example.com");
        let response = flow.start_device_flow();
        assert!(!response.device_code.is_empty());
    }

    #[test]
    fn token_store_creates() {
        let store = TokenStore::new("https://menzi.example.com");
        assert!(!store.is_authenticated());
    }

    #[test]
    fn token_store_saves_token() {
        let mut store = TokenStore::new("https://menzi.example.com");
        store.save_token(TokenResponse {
            access_token: "token-123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: Some("refresh-456".to_string()),
        });
        assert!(store.is_authenticated());
    }

    #[test]
    fn token_store_clears() {
        let mut store = TokenStore::new("https://menzi.example.com");
        store.save_token(TokenResponse {
            access_token: "token-123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: None,
        });
        store.clear();
        assert!(!store.is_authenticated());
    }

    #[test]
    fn device_flow_request_serializes() {
        let request = DeviceFlowRequest {
            client_id: "client-123".to_string(),
            scope: "read write".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("client-123"));
    }

    #[test]
    fn device_flow_response_deserializes() {
        let json = r#"{"device_code":"dev-123","user_code":"user-456","verification_uri":"https://menzi.example.com/device","expires_in":900,"interval":5}"#;
        let response: DeviceFlowResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.device_code, "dev-123");
    }

    #[test]
    fn token_response_deserializes() {
        let json = r#"{"access_token":"token-123","token_type":"Bearer","expires_in":3600,"refresh_token":"refresh-456"}"#;
        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "token-123");
    }

    #[test]
    fn token_store_serializes() {
        let store = TokenStore {
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-456".to_string()),
            expires_at: "2026-12-31T23:59:59Z".to_string(),
            server_url: "https://menzi.example.com".to_string(),
        };
        let json = serde_json::to_string(&store).unwrap();
        assert!(json.contains("token-123"));
    }
}
