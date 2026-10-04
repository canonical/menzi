use super::types::*;

pub struct PolicyEngine {
    configs: std::collections::HashMap<String, PolicyConfig>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            configs: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, project_id: String, config: PolicyConfig) {
        self.configs.insert(project_id, config);
    }

    pub fn is_model_allowed(&self, project_id: &str, model: &str) -> bool {
        let config = match self.configs.get(project_id) {
            Some(c) => c,
            None => return true,
        };

        for denied in &config.denied_models {
            if model_matches(model, denied) {
                return false;
            }
        }

        if config.allowed_models.is_empty() {
            return true;
        }

        for allowed in &config.allowed_models {
            if model_matches(model, allowed) {
                return true;
            }
        }

        false
    }

    pub fn get_classification(&self, project_id: &str) -> Option<DataClassification> {
        self.configs.get(project_id).map(|c| c.data_classification)
    }

    pub fn export(&self) -> Vec<(String, PolicyConfig)> {
        self.configs
            .iter()
            .map(|(scope, config)| (scope.clone(), config.clone()))
            .collect()
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn model_matches(model: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with("/*") {
        let prefix = pattern.strip_suffix("/*").unwrap_or(pattern);
        return model.starts_with(prefix);
    }
    if pattern.starts_with("*/") {
        let suffix = pattern.strip_prefix("*/").unwrap_or(pattern);
        let suffix_no_wildcard = suffix.trim_end_matches('*');
        return model.contains(suffix_no_wildcard);
    }
    model == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_allows_all_when_no_config() {
        let engine = PolicyEngine::new();
        assert!(engine.is_model_allowed("proj-1", "gpt-4"));
    }

    #[test]
    fn policy_blocks_denied_models() {
        let mut engine = PolicyEngine::new();
        engine.register(
            "proj-1".to_string(),
            PolicyConfig {
                allowed_models: vec!["*".to_string()],
                denied_models: vec!["*/preview-*".to_string()],
                data_classification: DataClassification::Internal,
            },
        );
        assert!(!engine.is_model_allowed("proj-1", "openrouter/anthropic/preview-claude"));
        assert!(engine.is_model_allowed("proj-1", "openrouter/anthropic/claude-sonnet-5"));
    }

    #[test]
    fn policy_allows_only_matching_models() {
        let mut engine = PolicyEngine::new();
        engine.register(
            "proj-1".to_string(),
            PolicyConfig {
                allowed_models: vec!["openrouter/anthropic/*".to_string()],
                denied_models: vec![],
                data_classification: DataClassification::Internal,
            },
        );
        assert!(engine.is_model_allowed("proj-1", "openrouter/anthropic/claude-sonnet-5"));
        assert!(!engine.is_model_allowed("proj-1", "openai/gpt-4"));
    }

    #[test]
    fn model_matches_exact() {
        assert!(model_matches("gpt-4", "gpt-4"));
    }

    #[test]
    fn model_matches_wildcard() {
        assert!(model_matches(
            "openrouter/anthropic/claude-sonnet-5",
            "openrouter/anthropic/*"
        ));
    }

    #[test]
    fn model_matches_global_wildcard() {
        assert!(model_matches("any-model", "*"));
    }

    #[test]
    fn model_does_not_match_different_prefix() {
        assert!(!model_matches("openai/gpt-4", "openrouter/anthropic/*"));
    }

    #[test]
    fn policy_returns_classification() {
        let mut engine = PolicyEngine::new();
        engine.register(
            "proj-1".to_string(),
            PolicyConfig {
                allowed_models: vec![],
                denied_models: vec![],
                data_classification: DataClassification::Restricted,
            },
        );
        assert_eq!(
            engine.get_classification("proj-1"),
            Some(DataClassification::Restricted)
        );
    }
}
