use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchCommand {
    pub project_id: String,
    pub branch: Option<String>,
    pub task: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewCommand {
    pub project_id: String,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub mode: PreviewMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardCommand {
    pub session_id: String,
    pub exposure_name: String,
    pub local_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCommand {
    pub project_id: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigCommand {
    pub action: ConfigAction,
    pub key: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigAction {
    Get,
    Set,
    List,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableOutput {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonOutput {
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlainOutput {
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct CommandRunner {
    pub server_url: String,
    pub token: String,
    pub output: OutputConfig,
}

impl CommandRunner {
    pub fn new(
        server_url: impl Into<String>,
        token: impl Into<String>,
        output: OutputConfig,
    ) -> Self {
        Self {
            server_url: server_url.into(),
            token: token.into(),
            output,
        }
    }

    pub fn format_output(&self, data: &serde_json::Value) -> String {
        match self.output.format {
            OutputFormat::Json => serde_json::to_string_pretty(data).unwrap_or_default(),
            OutputFormat::Plain => data.to_string(),
            OutputFormat::Table => {
                if let Some(obj) = data.as_object() {
                    obj.iter()
                        .map(|(k, v)| {
                            let val = match v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            format!("{}: {}", k, val)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    data.to_string()
                }
            }
        }
    }

    pub fn should_show_verbose(&self) -> bool {
        self.output.verbose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_command_serializes() {
        let cmd = LaunchCommand {
            project_id: "proj-1".to_string(),
            branch: Some("main".to_string()),
            task: Some("Add feature".to_string()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("proj-1"));
    }

    #[test]
    fn preview_command_serializes() {
        let cmd = PreviewCommand {
            project_id: "proj-1".to_string(),
            commit_sha: Some("abc123".to_string()),
            branch: Some("feature/x".to_string()),
            mode: PreviewMode::Pinned,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("abc123"));
    }

    #[test]
    fn forward_command_serializes() {
        let cmd = ForwardCommand {
            session_id: "sess-1".to_string(),
            exposure_name: "daemon-api".to_string(),
            local_port: 7000,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("daemon-api"));
    }

    #[test]
    fn design_command_serializes() {
        let cmd = DesignCommand {
            project_id: "proj-1".to_string(),
            revision: Some("latest".to_string()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("proj-1"));
    }

    #[test]
    fn config_command_serializes() {
        let cmd = ConfigCommand {
            action: ConfigAction::Get,
            key: Some("server_url".to_string()),
            value: None,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("server_url"));
    }

    #[test]
    fn config_action_roundtrips() {
        assert_eq!(
            serde_json::to_string(&ConfigAction::Get).unwrap(),
            "\"get\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigAction::Set).unwrap(),
            "\"set\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigAction::List).unwrap(),
            "\"list\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigAction::Delete).unwrap(),
            "\"delete\""
        );
    }

    #[test]
    fn output_config_serializes() {
        let config = OutputConfig {
            format: OutputFormat::Json,
            verbose: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"format\":\"json\""));
    }

    #[test]
    fn table_output_serializes() {
        let table = TableOutput {
            headers: vec!["Name".to_string(), "Status".to_string()],
            rows: vec![vec!["daemon".to_string(), "ready".to_string()]],
        };
        let json = serde_json::to_string(&table).unwrap();
        assert!(json.contains("daemon"));
    }

    #[test]
    fn command_runner_creates() {
        let runner = CommandRunner::new(
            "https://menzi.example.com",
            "token-123",
            OutputConfig {
                format: OutputFormat::Json,
                verbose: false,
            },
        );
        assert_eq!(runner.server_url, "https://menzi.example.com");
    }

    #[test]
    fn command_runner_formats_json() {
        let runner = CommandRunner::new(
            "https://menzi.example.com",
            "token-123",
            OutputConfig {
                format: OutputFormat::Json,
                verbose: false,
            },
        );
        let data = serde_json::json!({"key": "value"});
        let output = runner.format_output(&data);
        assert!(output.contains("\"key\": \"value\""));
    }

    #[test]
    fn command_runner_formats_table() {
        let runner = CommandRunner::new(
            "https://menzi.example.com",
            "token-123",
            OutputConfig {
                format: OutputFormat::Table,
                verbose: false,
            },
        );
        let data = serde_json::json!({"key": "value"});
        let output = runner.format_output(&data);
        assert!(output.contains("key: value"));
    }

    #[test]
    fn command_runner_checks_verbose() {
        let runner = CommandRunner::new(
            "https://menzi.example.com",
            "token-123",
            OutputConfig {
                format: OutputFormat::Json,
                verbose: true,
            },
        );
        assert!(runner.should_show_verbose());
    }

    #[test]
    fn json_output_serializes() {
        let output = JsonOutput {
            data: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("value"));
    }

    #[test]
    fn plain_output_serializes() {
        let output = PlainOutput {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("Hello"));
    }
}
