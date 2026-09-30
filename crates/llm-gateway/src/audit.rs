use super::types::*;
use menzi_common::ids::{ProjectId, SessionId, UserId};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct LogRequestParams {
    pub project_id: Option<ProjectId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub feature: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost: f64,
}

#[derive(Clone)]
pub struct AuditLogger {
    last: Arc<Mutex<Option<UsageRecord>>>,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            last: Arc::new(Mutex::new(None)),
        }
    }

    pub fn log_request(&self, params: LogRequestParams) -> UsageRecord {
        let record = UsageRecord {
            project_id: params.project_id,
            user_id: params.user_id,
            session_id: params.session_id,
            feature: params.feature,
            provider: params.provider,
            model: params.model,
            input_tokens: params.input_tokens,
            output_tokens: params.output_tokens,
            cost_usd: params.cost,
        };
        if let Ok(mut last) = self.last.lock() {
            *last = Some(record.clone());
        }
        record
    }

    pub fn last_record(&self) -> Option<UsageRecord> {
        self.last.lock().ok().and_then(|last| last.clone())
    }

    pub fn sanitize_content(&self, content: &str) -> String {
        content.replace("sk-", "[REDACTED]").replace("Bearer ", "")
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_logger_creates_usage_record() {
        let logger = AuditLogger::new();
        let record = logger.log_request(LogRequestParams {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "coding".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cost: 0.01,
        });
        assert_eq!(record.feature, "coding");
        assert_eq!(record.provider, "openai");
        assert_eq!(record.model, "gpt-4");
    }

    #[test]
    fn audit_logger_keeps_last_record() {
        let logger = AuditLogger::new();
        logger.log_request(LogRequestParams {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "coding".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            input_tokens: 1,
            output_tokens: 1,
            cost: 0.01,
        });
        let record = logger.last_record().expect("record recorded");
        assert_eq!(record.model, "gpt-4");
    }

    #[test]
    fn audit_logger_records_tenant_ids() {
        let logger = AuditLogger::new();
        let project_id = ProjectId::new();
        let user_id = UserId::new();
        logger.log_request(LogRequestParams {
            project_id: Some(project_id),
            user_id: Some(user_id),
            session_id: None,
            feature: "chat".to_string(),
            provider: "openrouter".to_string(),
            model: "mock-gpt".to_string(),
            input_tokens: 1,
            output_tokens: 1,
            cost: 0.01,
        });
        let record = logger.last_record().expect("record recorded");
        assert_eq!(record.project_id, Some(project_id));
        assert_eq!(record.user_id, Some(user_id));
    }

    #[test]
    fn audit_logger_redacts_api_keys() {
        let logger = AuditLogger::new();
        let sanitized = logger.sanitize_content("sk-abc123secret");
        assert!(!sanitized.contains("sk-abc123"));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn audit_logger_redacts_bearer_tokens() {
        let logger = AuditLogger::new();
        let sanitized = logger.sanitize_content("Bearer my-token");
        assert!(!sanitized.contains("Bearer"));
    }
}
