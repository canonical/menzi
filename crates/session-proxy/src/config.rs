use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub allowlist: Vec<String>,
    pub deny_list: Vec<String>,
}

impl ProxyConfig {
    pub fn default_allowlist() -> Self {
        Self {
            allowlist: vec![
                "GET /session".to_string(),
                "POST /session".to_string(),
                "GET /session/:id".to_string(),
                "DELETE /session/:id".to_string(),
                "GET /session/:id/message".to_string(),
                "POST /session/:id/message".to_string(),
                "GET /session/:id/diff".to_string(),
                "POST /session/:id/interrupt".to_string(),
                "GET /vcs".to_string(),
                "GET /vcs/status".to_string(),
                "GET /agent".to_string(),
                "GET /api/session".to_string(),
                "POST /api/session".to_string(),
                "GET /api/session/:id".to_string(),
                "GET /api/session/:id/message".to_string(),
                "POST /api/session/:id/prompt".to_string(),
                "GET /api/session/:id/log".to_string(),
                "POST /api/session/:id/interrupt".to_string(),
                "GET /api/session/:id/diff".to_string(),
                "GET /api/vcs/status".to_string(),
                "GET /api/vcs/diff".to_string(),
                "GET /api/model".to_string(),
                "GET /api/agent".to_string(),
                "GET /api/event".to_string(),
                "GET /event".to_string(),
                "POST /api/session/:id/switchModel".to_string(),
                "POST /api/session/:id/switchAgent".to_string(),
                "GET /api/fs/*".to_string(),
                "POST /api/pty/*".to_string(),
                "GET /api/config".to_string(),
            ],
            deny_list: vec![
                "POST /api/config".to_string(),
                "POST /api/auth/*".to_string(),
                "POST /api/plugin/*".to_string(),
                "POST /api/mcp/*".to_string(),
            ],
        }
    }

    pub fn is_allowed(&self, method: &str, path: &str) -> bool {
        let route = format!("{} {}", method, path);
        for denied in &self.deny_list {
            if route_matches(&route, denied) {
                return false;
            }
        }
        for allowed in &self.allowlist {
            if route_matches(&route, allowed) {
                return true;
            }
        }
        false
    }
}

fn route_matches(route: &str, pattern: &str) -> bool {
    let route_parts: Vec<&str> = route.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    let route_method = route_parts[0];
    let pattern_method = pattern_parts[0];
    if route_method != pattern_method {
        return false;
    }

    let route_path_parts = &route_parts[1..];
    let pattern_path_parts = &pattern_parts[1..];

    if pattern_path_parts.len() == 1 && pattern_path_parts[0] == "*" {
        return true;
    }

    if route_path_parts.len() < pattern_path_parts.len() {
        return false;
    }

    for (i, pattern_part) in pattern_path_parts.iter().enumerate() {
        if pattern_part.starts_with(':') {
            continue;
        }
        if pattern_part == &"*" {
            return true;
        }
        if route_path_parts[i] != *pattern_part {
            return false;
        }
    }

    route_path_parts.len() == pattern_path_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_allowlist_contains_session_routes() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.allowlist.contains(&"GET /api/session".to_string()));
        assert!(config.allowlist.contains(&"POST /api/session".to_string()));
        assert!(config
            .allowlist
            .contains(&"GET /api/session/:id/message".to_string()));
        assert!(config.allowlist.contains(&"GET /api/event".to_string()));
    }

    #[test]
    fn sdk_session_routes_are_allowed() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.is_allowed("GET", "/session"));
        assert!(config.is_allowed("POST", "/session"));
        assert!(config.is_allowed("GET", "/session/abc/message"));
        assert!(config.is_allowed("POST", "/session/abc/message"));
        assert!(config.is_allowed("POST", "/session/abc/interrupt"));
        assert!(config.is_allowed("GET", "/session/abc/diff"));
        assert!(config.is_allowed("GET", "/vcs/status"));
        assert!(config.is_allowed("GET", "/agent"));
        assert!(config.is_allowed("GET", "/event"));
    }

    #[test]
    fn session_message_history_is_allowed() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.is_allowed("GET", "/api/session/abc/message"));
        assert!(config.is_allowed("GET", "/api/session/abc"));
        assert!(config.is_allowed("GET", "/api/event"));
    }

    #[test]
    fn default_deny_list_contains_config_routes() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.deny_list.contains(&"POST /api/config".to_string()));
        assert!(config.deny_list.contains(&"POST /api/auth/*".to_string()));
    }

    #[test]
    fn is_allowed_returns_true_for_allowed_route() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.is_allowed("GET", "/api/session"));
    }

    #[test]
    fn is_allowed_returns_false_for_denied_route() {
        let config = ProxyConfig::default_allowlist();
        assert!(!config.is_allowed("POST", "/api/config"));
    }

    #[test]
    fn is_allowed_returns_false_for_unknown_route() {
        let config = ProxyConfig::default_allowlist();
        assert!(!config.is_allowed("DELETE", "/api/unknown"));
    }

    #[test]
    fn is_allowed_matches_parameterized_routes() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.is_allowed("POST", "/api/session/abc123/prompt"));
        assert!(config.is_allowed("GET", "/api/session/abc123/log"));
    }

    #[test]
    fn is_allowed_matches_wildcard_routes() {
        let config = ProxyConfig::default_allowlist();
        assert!(config.is_allowed("GET", "/api/fs/some/path"));
        assert!(config.is_allowed("POST", "/api/pty/abc"));
    }

    #[test]
    fn route_matches_exact() {
        assert!(route_matches("GET /api/session", "GET /api/session"));
    }

    #[test]
    fn route_matches_parameterized() {
        assert!(route_matches(
            "POST /api/session/abc/prompt",
            "POST /api/session/:id/prompt"
        ));
    }

    #[test]
    fn route_matches_wildcard() {
        assert!(route_matches("GET /api/fs/some/path", "GET /api/fs/*"));
    }

    #[test]
    fn route_does_not_match_different_method() {
        assert!(!route_matches("POST /api/session", "GET /api/session"));
    }

    #[test]
    fn route_does_not_match_different_path() {
        assert!(!route_matches("GET /api/other", "GET /api/session"));
    }

    #[test]
    fn proxy_config_serializes() {
        let config = ProxyConfig::default_allowlist();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("allowlist"));
        assert!(json.contains("deny_list"));
    }
}
