use menzi_common::ids::{ProjectId, SessionId, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: i32,
    pub delta: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    pub model: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub model: String,
    pub embedding: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub encrypted_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub allowed_models: Vec<String>,
    pub denied_models: Vec<String>,
    pub data_classification: DataClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub project_id: Option<ProjectId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub feature: String,
    pub spent_usd: f64,
    pub budget_usd: f64,
}

impl BudgetStatus {
    pub fn is_exceeded(&self) -> bool {
        self.spent_usd >= self.budget_usd
    }

    pub fn is_warning(&self) -> bool {
        self.spent_usd >= self.budget_usd * 0.8
    }

    pub fn remaining(&self) -> f64 {
        (self.budget_usd - self.spent_usd).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub project_id: Option<ProjectId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub feature: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_serializes() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            stream: false,
            max_tokens: Some(100),
            temperature: Some(0.7),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"stream\":false"));
    }

    #[test]
    fn chat_response_deserializes() {
        let json = r#"{"id":"chat-1","model":"gpt-4","choices":[{"index":0,"message":{"role":"assistant","content":"Hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let response: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "chat-1");
        assert_eq!(response.usage.total_tokens, 15);
    }

    #[test]
    fn chat_chunk_deserializes() {
        let json = r#"{"id":"chat-1","model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hi"},"finish_reason":null}]}"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].delta.content, "Hi");
    }

    #[test]
    fn embed_request_serializes() {
        let request = EmbedRequest {
            model: "text-embedding-3-small".to_string(),
            input: "Hello world".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"text-embedding-3-small\""));
    }

    #[test]
    fn embed_response_deserializes() {
        let json = r#"{"model":"text-embedding-3-small","embedding":[0.1,0.2,0.3]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embedding.len(), 3);
    }

    #[test]
    fn budget_status_not_exceeded_under_budget() {
        let budget = BudgetStatus {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "test".to_string(),
            spent_usd: 85.0,
            budget_usd: 100.0,
        };
        assert!(!budget.is_exceeded());
        assert!(budget.is_warning());
        assert_eq!(budget.remaining(), 15.0);
    }

    #[test]
    fn budget_status_exceeded_at_budget() {
        let budget = BudgetStatus {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "test".to_string(),
            spent_usd: 100.0,
            budget_usd: 100.0,
        };
        assert!(budget.is_exceeded());
        assert_eq!(budget.remaining(), 0.0);
    }

    #[test]
    fn budget_status_not_warning_under_80_percent() {
        let budget = BudgetStatus {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "test".to_string(),
            spent_usd: 79.0,
            budget_usd: 100.0,
        };
        assert!(!budget.is_warning());
    }

    #[test]
    fn data_classification_roundtrips() {
        assert_eq!(
            serde_json::to_string(&DataClassification::Public).unwrap(),
            "\"public\""
        );
        assert_eq!(
            serde_json::to_string(&DataClassification::Internal).unwrap(),
            "\"internal\""
        );
        assert_eq!(
            serde_json::to_string(&DataClassification::Restricted).unwrap(),
            "\"restricted\""
        );
    }

    #[test]
    fn usage_record_serializes() {
        let record = UsageRecord {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "coding".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: 0.01,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"provider\":\"openai\""));
        assert!(json.contains("\"model\":\"gpt-4\""));
    }

    #[test]
    fn provider_info_serializes() {
        let info = ProviderInfo {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"id\":\"openai\""));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn credential_serializes() {
        let cred = Credential {
            id: "cred-1".to_string(),
            provider: "openai".to_string(),
            name: "My Key".to_string(),
            encrypted_key: "encrypted".to_string(),
        };
        let json = serde_json::to_string(&cred).unwrap();
        assert!(json.contains("\"id\":\"cred-1\""));
        assert!(json.contains("\"provider\":\"openai\""));
    }

    #[test]
    fn policy_config_serializes() {
        let policy = PolicyConfig {
            allowed_models: vec!["gpt-4".to_string()],
            denied_models: vec!["gpt-3.5".to_string()],
            data_classification: DataClassification::Internal,
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("\"data_classification\":\"internal\""));
    }
}
