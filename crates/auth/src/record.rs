use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRecord {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

impl SessionRecord {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSession {
    pub user_id: Uuid,
    pub token_hash: String,
    pub csrf_hash: String,
    pub provider_id: String,
    pub expires_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityLink {
    pub user_id: Uuid,
    pub provider_id: String,
    pub subject: String,
    pub email_at_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcState {
    pub state: String,
    pub provider_id: String,
    pub nonce: String,
    pub code_verifier: String,
    pub redirect_to: Option<String>,
    pub expires_at: DateTime<Utc>,
}

impl OidcState {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordReset {
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub fn normalise_email(email: &str) -> String {
    email.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserRecord {
        UserRecord {
            id: Uuid::new_v4(),
            org_id: None,
            email: "person@example.com".to_string(),
            name: "Person".to_string(),
            avatar_url: None,
            password_hash: None,
            email_verified_at: None,
            created_at: Utc::now(),
        }
    }

    fn session(expires_at: DateTime<Utc>) -> SessionRecord {
        SessionRecord {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            provider_id: "password".to_string(),
            created_at: Utc::now(),
            expires_at,
            last_seen_at: Utc::now(),
            user_agent: None,
            ip: None,
        }
    }

    #[test]
    fn a_user_record_carries_no_org_when_registered() {
        assert!(user().org_id.is_none());
    }

    #[test]
    fn a_session_reports_its_own_expiry() {
        let now = Utc::now();
        assert!(!session(now + chrono::Duration::hours(1)).is_expired(now));
        assert!(session(now - chrono::Duration::hours(1)).is_expired(now));
    }

    #[test]
    fn an_oidc_state_reports_its_own_expiry() {
        let now = Utc::now();
        let state = OidcState {
            state: "s".to_string(),
            provider_id: "google".to_string(),
            nonce: "n".to_string(),
            code_verifier: "v".to_string(),
            redirect_to: None,
            expires_at: now + chrono::Duration::minutes(10),
        };
        assert!(!state.is_expired(now));
        let expired = OidcState {
            expires_at: now - chrono::Duration::minutes(1),
            ..state
        };
        assert!(expired.is_expired(now));
    }

    #[test]
    fn email_is_trimmed_and_lowercased() {
        assert_eq!(normalise_email("  Bob@Example.COM "), "bob@example.com");
        assert_eq!(normalise_email("A@B"), "a@b");
    }
}
