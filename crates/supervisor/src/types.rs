pub use menzi_common::ids::SessionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    pub opencode_version: String,
    pub control_plane_url: String,
    pub heartbeat_interval_secs: u64,
    pub session_token: String,
    pub git_token: String,
}

impl SupervisorConfig {
    pub fn from_env() -> menzi_common::Result<Self> {
        let opencode_version =
            std::env::var("MENZI_OPENCODE_VERSION").unwrap_or_else(|_| "2.0.0".to_string());
        let control_plane_url = std::env::var("MENZI_CONTROL_PLANE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let heartbeat_interval_secs = std::env::var("MENZI_HEARTBEAT_INTERVAL_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(30);
        let session_token =
            std::env::var("MENZI_SESSION_TOKEN").unwrap_or_else(|_| "dev-token".to_string());
        let git_token = std::env::var("MENZI_GIT_TOKEN").unwrap_or_default();
        Ok(Self {
            opencode_version,
            control_plane_url,
            heartbeat_interval_secs,
            session_token,
            git_token,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpencodeConfig {
    pub providers: std::collections::HashMap<String, ProviderConfig>,
    pub model: String,
    pub mcp: McpConfig,
    pub permissions: Vec<PermissionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub package: String,
    pub settings: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: std::collections::HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    #[serde(rename = "type")]
    pub server_type: String,
    pub command: Vec<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,
    pub resource: String,
    pub effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub session_id: SessionId,
    pub status: String,
    pub timestamp: String,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_mb: u64,
    pub disk_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEnvironment {
    pub components: Vec<EnvComponent>,
    pub last_relaunch_result: Option<RelaunchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvComponent {
    pub name: String,
    pub status: String,
    pub exposed_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelaunchResult {
    pub success: bool,
    pub restarted_components: Vec<String>,
    pub skipped_components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionEntry {
    pub id: String,
    pub content: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticMessage {
    pub content: String,
    pub message_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn supervisor_config_serializes() {
        let config = SupervisorConfig {
            opencode_version: "2.0.0".to_string(),
            control_plane_url: "https://control.example.com".to_string(),
            heartbeat_interval_secs: 30,
            session_token: "token-123".to_string(),
            git_token: "git-token-456".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"opencode_version\":\"2.0.0\""));
        assert!(json.contains("\"heartbeat_interval_secs\":30"));
    }

    #[test]
    fn supervisor_config_from_env_uses_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("MENZI_OPENCODE_VERSION");
        std::env::remove_var("MENZI_CONTROL_PLANE_URL");
        std::env::remove_var("MENZI_HEARTBEAT_INTERVAL_SECS");
        std::env::remove_var("MENZI_SESSION_TOKEN");
        std::env::remove_var("MENZI_GIT_TOKEN");

        let config = SupervisorConfig::from_env().unwrap();
        assert_eq!(config.opencode_version, "2.0.0");
        assert_eq!(config.control_plane_url, "http://127.0.0.1:8080");
        assert_eq!(config.heartbeat_interval_secs, 30);
        assert_eq!(config.session_token, "dev-token");
        assert_eq!(config.git_token, "");
    }

    #[test]
    fn supervisor_config_from_env_overrides_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("MENZI_OPENCODE_VERSION", "2.1.0");
        std::env::set_var("MENZI_CONTROL_PLANE_URL", "https://control.example.com");
        std::env::set_var("MENZI_HEARTBEAT_INTERVAL_SECS", "15");
        std::env::set_var("MENZI_SESSION_TOKEN", "token-abc");
        std::env::set_var("MENZI_GIT_TOKEN", "git-token-xyz");

        let config = SupervisorConfig::from_env().unwrap();
        assert_eq!(config.opencode_version, "2.1.0");
        assert_eq!(config.control_plane_url, "https://control.example.com");
        assert_eq!(config.heartbeat_interval_secs, 15);
        assert_eq!(config.session_token, "token-abc");
        assert_eq!(config.git_token, "git-token-xyz");

        std::env::remove_var("MENZI_OPENCODE_VERSION");
        std::env::remove_var("MENZI_CONTROL_PLANE_URL");
        std::env::remove_var("MENZI_HEARTBEAT_INTERVAL_SECS");
        std::env::remove_var("MENZI_SESSION_TOKEN");
        std::env::remove_var("MENZI_GIT_TOKEN");
    }

    #[test]
    fn opencode_config_serializes() {
        let mut providers = std::collections::HashMap::new();
        let mut settings = std::collections::HashMap::new();
        settings.insert(
            "baseURL".to_string(),
            "https://llm-gw.example.com/v1".to_string(),
        );
        settings.insert("apiKey".to_string(), "{env:MENZI_LLM_TOKEN}".to_string());
        providers.insert(
            "openrouter".to_string(),
            ProviderConfig {
                package: "aisdk:@ai-sdk/openai-compatible".to_string(),
                settings,
            },
        );

        let mut servers = std::collections::HashMap::new();
        servers.insert(
            "design".to_string(),
            McpServerConfig {
                server_type: "local".to_string(),
                command: vec![
                    "menzi-supervisor".to_string(),
                    "mcp".to_string(),
                    "design".to_string(),
                ],
                disabled: false,
            },
        );

        let config = OpencodeConfig {
            providers,
            model: "openrouter/anthropic/claude-sonnet-5".to_string(),
            mcp: McpConfig { servers },
            permissions: vec![PermissionRule {
                action: "design_*".to_string(),
                resource: "*".to_string(),
                effect: "allow".to_string(),
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("openrouter"));
        assert!(json.contains("design"));
    }

    #[test]
    fn heartbeat_serializes() {
        let heartbeat = Heartbeat {
            session_id: SessionId::new(),
            status: "running".to_string(),
            timestamp: "2026-09-28T00:00:00Z".to_string(),
            resource_usage: ResourceUsage {
                cpu_percent: 45.5,
                memory_mb: 1024,
                disk_mb: 2048,
            },
        };
        let json = serde_json::to_string(&heartbeat).unwrap();
        assert!(json.contains("\"status\":\"running\""));
        assert!(json.contains("\"cpu_percent\":45.5"));
    }

    #[test]
    fn resource_usage_serializes() {
        let usage = ResourceUsage {
            cpu_percent: 75.0,
            memory_mb: 512,
            disk_mb: 1024,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"cpu_percent\":75.0"));
    }

    #[test]
    fn session_environment_serializes() {
        let env = SessionEnvironment {
            components: vec![EnvComponent {
                name: "daemon".to_string(),
                status: "ready".to_string(),
                exposed_url: Some("https://daemon-s-123.dev.example.com".to_string()),
            }],
            last_relaunch_result: Some(RelaunchResult {
                success: true,
                restarted_components: vec!["daemon".to_string()],
                skipped_components: vec!["db".to_string()],
            }),
        };
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("daemon"));
        assert!(json.contains("ready"));
    }

    #[test]
    fn instruction_entry_serializes() {
        let entry = InstructionEntry {
            id: "env-status".to_string(),
            content: "db: ready, daemon: ready".to_string(),
            priority: 1,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"id\":\"env-status\""));
    }

    #[test]
    fn synthetic_message_serializes() {
        let msg = SyntheticMessage {
            content: "Build failed for daemon-bin".to_string(),
            message_type: "error".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Build failed"));
    }

    #[test]
    fn relaunch_result_serializes() {
        let result = RelaunchResult {
            success: true,
            restarted_components: vec!["daemon".to_string()],
            skipped_components: vec!["db".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn permission_rule_serializes() {
        let rule = PermissionRule {
            action: "shell".to_string(),
            resource: "git push *".to_string(),
            effect: "ask".to_string(),
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("\"action\":\"shell\""));
        assert!(json.contains("\"effect\":\"ask\""));
    }
}
