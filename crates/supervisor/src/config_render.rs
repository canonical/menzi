use crate::types::*;

pub fn render_opencode_config(config: &OpencodeConfig) -> menzi_common::Result<String> {
    serde_json::to_string_pretty(config).map_err(|error| {
        menzi_common::MenziError::Internal(anyhow::Error::msg(format!(
            "failed to render opencode config: {error}"
        )))
    })
}

pub struct ConfigRenderer {
    pub gateway_url: String,
    pub session_token: String,
}

impl ConfigRenderer {
    pub fn new(gateway_url: impl Into<String>, session_token: impl Into<String>) -> Self {
        Self {
            gateway_url: gateway_url.into(),
            session_token: session_token.into(),
        }
    }

    pub fn render(&self, model: &str, permissions: Vec<PermissionRule>) -> OpencodeConfig {
        let mut providers = std::collections::HashMap::new();
        let mut settings = std::collections::HashMap::new();
        settings.insert(
            "baseURL".to_string(),
            format!("{}/v1/p/openrouter", self.gateway_url),
        );
        settings.insert("apiKey".to_string(), "{env:MENZI_LLM_TOKEN}".to_string());
        providers.insert(
            "openrouter".to_string(),
            ProviderConfig {
                package: "aisdk:@ai-sdk/openai-compatible".to_string(),
                settings,
            },
        );

        let mut local_settings = std::collections::HashMap::new();
        local_settings.insert(
            "baseURL".to_string(),
            format!("{}/v1/p/local", self.gateway_url),
        );
        local_settings.insert("apiKey".to_string(), "{env:MENZI_LLM_TOKEN}".to_string());
        providers.insert(
            "local".to_string(),
            ProviderConfig {
                package: "aisdk:@ai-sdk/openai-compatible".to_string(),
                settings: local_settings,
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
        servers.insert(
            "env".to_string(),
            McpServerConfig {
                server_type: "local".to_string(),
                command: vec![
                    "menzi-supervisor".to_string(),
                    "mcp".to_string(),
                    "env".to_string(),
                ],
                disabled: false,
            },
        );

        OpencodeConfig {
            providers,
            model: model.to_string(),
            mcp: McpConfig { servers },
            permissions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_renderer_creates_opencode_config() {
        let renderer = ConfigRenderer::new("https://llm-gw.example.com", "token-123");
        let config = renderer.render("openrouter/anthropic/claude-sonnet-5", vec![]);
        assert_eq!(config.model, "openrouter/anthropic/claude-sonnet-5");
        assert!(config.providers.contains_key("openrouter"));
        assert!(config.providers.contains_key("local"));
    }

    #[test]
    fn config_renderer_includes_mcp_servers() {
        let renderer = ConfigRenderer::new("https://llm-gw.example.com", "token-123");
        let config = renderer.render("openrouter/anthropic/claude-sonnet-5", vec![]);
        assert!(config.mcp.servers.contains_key("design"));
        assert!(config.mcp.servers.contains_key("env"));
    }

    #[test]
    fn config_renderer_includes_permissions() {
        let renderer = ConfigRenderer::new("https://llm-gw.example.com", "token-123");
        let permissions = vec![PermissionRule {
            action: "shell".to_string(),
            resource: "git push *".to_string(),
            effect: "ask".to_string(),
        }];
        let config = renderer.render("openrouter/anthropic/claude-sonnet-5", permissions);
        assert_eq!(config.permissions.len(), 1);
        assert_eq!(config.permissions[0].action, "shell");
    }

    #[test]
    fn render_opencode_config_roundtrips() {
        let renderer = ConfigRenderer::new("https://llm-gw.example.com", "token-123");
        let config = renderer.render("openrouter/anthropic/claude-sonnet-5", vec![]);
        let json = render_opencode_config(&config).unwrap();
        let parsed: OpencodeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model, config.model);
        assert!(parsed.providers.contains_key("openrouter"));
        assert!(parsed.mcp.servers.contains_key("design"));
    }

    #[test]
    fn render_opencode_config_includes_providers() {
        let renderer = ConfigRenderer::new("https://llm-gw.example.com", "token-123");
        let config = renderer.render("openrouter/anthropic/claude-sonnet-5", vec![]);
        let json = render_opencode_config(&config).unwrap();
        assert!(json.contains("\"providers\""));
        assert!(json.contains("openrouter"));
    }
}
