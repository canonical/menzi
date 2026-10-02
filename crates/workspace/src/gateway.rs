use async_trait::async_trait;
use menzi_common::{MenziError, Result};
use std::time::Duration;

use crate::types::{AgentSession, PromptOutcome};

#[async_trait]
pub trait OpencodeGateway: Send + Sync {
    async fn health(&self, endpoint: &str) -> Result<()>;
    async fn create_session(&self, endpoint: &str, title: Option<&str>) -> Result<AgentSession>;
    async fn prompt(&self, endpoint: &str, session_id: &str, text: &str) -> Result<PromptOutcome>;
    async fn interrupt(&self, endpoint: &str, session_id: &str) -> Result<()>;
    async fn sessions(&self, endpoint: &str) -> Result<Vec<AgentSession>>;
}

pub struct HttpOpencodeGateway {
    http: reqwest::Client,
    timeout: Duration,
}

impl HttpOpencodeGateway {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            http: reqwest::Client::new(),
            timeout,
        }
    }

    fn url(endpoint: &str, path: &str) -> String {
        format!("{}{}", endpoint.trim_end_matches('/'), path)
    }

    async fn send_json(&self, request: reqwest::RequestBuilder) -> Result<serde_json::Value> {
        let response = tokio::time::timeout(self.timeout, request.send())
            .await
            .map_err(|_| MenziError::Gateway("opencode timed out".to_string()))?
            .map_err(|error| MenziError::Gateway(format!("opencode unreachable: {error}")))?;
        let status = response.status();
        let body = tokio::time::timeout(self.timeout, response.text())
            .await
            .map_err(|_| MenziError::Gateway("opencode read timed out".to_string()))?
            .map_err(|error| MenziError::Gateway(format!("opencode read failed: {error}")))?;
        if !status.is_success() {
            return Err(MenziError::Gateway(format!("opencode {status}: {body}")));
        }
        if body.trim().is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&body)
            .map_err(|error| MenziError::Gateway(format!("invalid opencode response: {error}")))
    }
}

impl Default for HttpOpencodeGateway {
    fn default() -> Self {
        Self::new()
    }
}

fn message_id(value: &serde_json::Value) -> Option<String> {
    value
        .get("info")
        .and_then(|info| info.get("id"))
        .and_then(|id| id.as_str())
        .map(str::to_string)
}

fn finish_reason(value: &serde_json::Value) -> Option<String> {
    value
        .get("info")
        .and_then(|info| info.get("finish"))
        .and_then(|finish| finish.as_str())
        .map(str::to_string)
}

fn message_error(value: &serde_json::Value) -> Option<String> {
    let error = value
        .get("info")
        .and_then(|info| info.get("error"))
        .filter(|error| !error.is_null())?;
    let message = error
        .get("message")
        .and_then(|message| message.as_str())
        .or_else(|| {
            error
                .get("data")
                .and_then(|data| data.get("message"))
                .and_then(|message| message.as_str())
        });
    Some(message.unwrap_or("agent reported an error").to_string())
}

fn session_from(value: &serde_json::Value) -> Option<AgentSession> {
    let id = value.get("id").and_then(|id| id.as_str())?;
    Some(AgentSession {
        id: id.to_string(),
        title: value
            .get("title")
            .and_then(|title| title.as_str())
            .map(str::to_string),
        directory: value
            .get("directory")
            .and_then(|directory| directory.as_str())
            .map(str::to_string),
    })
}

#[async_trait]
impl OpencodeGateway for HttpOpencodeGateway {
    async fn health(&self, endpoint: &str) -> Result<()> {
        self.send_json(self.http.get(Self::url(endpoint, "/api/model")))
            .await?;
        Ok(())
    }

    async fn create_session(&self, endpoint: &str, title: Option<&str>) -> Result<AgentSession> {
        let mut body = serde_json::json!({});
        if let Some(title) = title {
            body["title"] = serde_json::Value::String(title.to_string());
        }
        let value = self
            .send_json(self.http.post(Self::url(endpoint, "/session")).json(&body))
            .await?;
        session_from(&value)
            .ok_or_else(|| MenziError::Gateway("opencode session has no id".to_string()))
    }

    async fn prompt(&self, endpoint: &str, session_id: &str, text: &str) -> Result<PromptOutcome> {
        let body = serde_json::json!({
            "parts": [{ "type": "text", "text": text }]
        });
        let value = self
            .send_json(
                self.http
                    .post(Self::url(
                        endpoint,
                        &format!("/session/{session_id}/message"),
                    ))
                    .json(&body),
            )
            .await?;
        Ok(PromptOutcome {
            session_id: session_id.to_string(),
            message_id: message_id(&value),
            finish_reason: finish_reason(&value),
            error: message_error(&value),
        })
    }

    async fn interrupt(&self, endpoint: &str, session_id: &str) -> Result<()> {
        self.send_json(
            self.http
                .post(Self::url(endpoint, &format!("/session/{session_id}/abort")))
                .json(&serde_json::json!({})),
        )
        .await?;
        Ok(())
    }

    async fn sessions(&self, endpoint: &str) -> Result<Vec<AgentSession>> {
        let value = self
            .send_json(self.http.get(Self::url(endpoint, "/session")))
            .await?;
        let list = value
            .as_array()
            .cloned()
            .or_else(|| value.get("data").and_then(|data| data.as_array()).cloned())
            .ok_or_else(|| {
                MenziError::Gateway("opencode session list is not a list".to_string())
            })?;
        Ok(list
            .iter()
            .filter(|session| session.get("parentID").and_then(|value| value.as_str()).is_none())
            .filter_map(session_from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_joins_without_doubling_slashes() {
        assert_eq!(
            HttpOpencodeGateway::url("http://wsp-a-b:17999", "/api/model"),
            "http://wsp-a-b:17999/api/model"
        );
        assert_eq!(
            HttpOpencodeGateway::url("http://wsp-a-b:17999/", "/api/model"),
            "http://wsp-a-b:17999/api/model"
        );
    }

    #[test]
    fn session_from_reads_id_and_title() {
        let value = serde_json::json!({ "id": "ses_1", "title": "refactor" });
        let session = session_from(&value).unwrap();
        assert_eq!(session.id, "ses_1");
        assert_eq!(session.title.as_deref(), Some("refactor"));
    }

    #[test]
    fn session_from_reads_the_working_directory() {
        let value = serde_json::json!({ "id": "ses_1", "directory": "/workspace" });
        assert_eq!(
            session_from(&value).unwrap().directory.as_deref(),
            Some("/workspace")
        );
    }

    #[test]
    fn session_from_tolerates_a_missing_directory() {
        let value = serde_json::json!({ "id": "ses_1" });
        assert_eq!(session_from(&value).unwrap().directory, None);
    }

    #[test]
    fn session_from_rejects_a_response_without_an_id() {
        assert!(session_from(&serde_json::json!({ "title": "no id" })).is_none());
    }

    #[test]
    fn prompt_outcome_reads_a_finished_message() {
        let value = serde_json::json!({
            "info": { "id": "msg_1", "finish": "stop" },
            "parts": []
        });
        assert_eq!(message_id(&value).as_deref(), Some("msg_1"));
        assert_eq!(finish_reason(&value).as_deref(), Some("stop"));
        assert!(message_error(&value).is_none());
    }

    #[test]
    fn prompt_outcome_reports_an_error_inside_a_success_response() {
        let value = serde_json::json!({
            "info": {
                "id": "msg_2",
                "finish": "error",
                "error": { "name": "ProviderAuthError", "data": { "message": "budget exceeded" } }
            }
        });
        let error = message_error(&value).unwrap();
        assert!(error.contains("budget exceeded"), "{error}");
    }

    #[test]
    fn prompt_outcome_handles_a_string_error() {
        let value = serde_json::json!({
            "info": { "id": "msg_3", "error": "plain failure" }
        });
        assert_eq!(
            message_error(&value).as_deref(),
            Some("agent reported an error")
        );
    }

    #[test]
    fn prompt_outcome_ignores_a_null_error() {
        let value = serde_json::json!({ "info": { "id": "msg_4", "error": null } });
        assert!(message_error(&value).is_none());
    }
}
