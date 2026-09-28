use chrono::{DateTime, Utc};
use menzi_common::ids::{ProjectId, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToken {
    pub token: String,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_token_serializes() {
        let token = SessionToken {
            token: "abc123".to_string(),
            user_id: UserId::new(),
            project_id: ProjectId::new(),
            expires_at: Utc::now(),
        };
        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("\"token\":\"abc123\""));
    }
}
