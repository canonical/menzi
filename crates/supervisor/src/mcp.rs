use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignOverview {
    pub clauses: Vec<ClauseSummary>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClauseSummary {
    pub id: String,
    pub title: String,
    pub level: String,
    pub status: String,
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignClausesForPaths {
    pub clauses: Vec<ClauseDetail>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClauseDetail {
    pub id: String,
    pub title: String,
    pub level: String,
    pub status: String,
    pub statement: String,
    pub verification: serde_json::Value,
    pub waivers: Vec<WaiverInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaiverInfo {
    pub id: String,
    pub paths: Vec<String>,
    pub reason: String,
    pub until: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignSearchResult {
    pub results: Vec<SearchResultItem>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub clause_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCheckResult {
    pub findings: Vec<CheckFinding>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckFinding {
    pub clause_id: String,
    pub verdict: String,
    pub file: String,
    pub line: i32,
    pub message: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmendmentDraft {
    pub amendment_id: String,
    pub clause_id: Option<String>,
    pub action: String,
    pub redline_diff: serde_json::Value,
    pub status: String,
    pub drafted_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaiverDraft {
    pub clause_id: String,
    pub paths: Vec<String>,
    pub reason: String,
    pub until: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct McpProxy {
    pub design_service_url: String,
    pub session_token: String,
    pub cached_revision: Option<String>,
    pub cached_clauses: Vec<ClauseSummary>,
}

impl McpProxy {
    pub fn new(design_service_url: impl Into<String>, session_token: impl Into<String>) -> Self {
        Self {
            design_service_url: design_service_url.into(),
            session_token: session_token.into(),
            cached_revision: None,
            cached_clauses: Vec::new(),
        }
    }

    pub fn cache_revision(&mut self, revision: String) {
        self.cached_revision = Some(revision);
    }

    pub fn cache_clauses(&mut self, clauses: Vec<ClauseSummary>) {
        self.cached_clauses = clauses;
    }

    pub fn get_cached_revision(&self) -> Option<&str> {
        self.cached_revision.as_deref()
    }

    pub fn clear_cache(&mut self) {
        self.cached_revision = None;
        self.cached_clauses.clear();
    }

    pub fn is_cache_valid(&self) -> bool {
        self.cached_revision.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_proxy_stores_config() {
        let proxy = McpProxy::new("https://design.example.com", "token-123");
        assert_eq!(proxy.design_service_url, "https://design.example.com");
        assert_eq!(proxy.session_token, "token-123");
    }

    #[test]
    fn mcp_proxy_caches_revision() {
        let mut proxy = McpProxy::new("https://design.example.com", "token-123");
        proxy.cache_revision("R42".to_string());
        assert_eq!(proxy.get_cached_revision(), Some("R42"));
    }

    #[test]
    fn mcp_proxy_caches_clauses() {
        let mut proxy = McpProxy::new("https://design.example.com", "token-123");
        proxy.cache_revision("R42".to_string());
        proxy.cache_clauses(vec![ClauseSummary {
            id: "DC-12".to_string(),
            title: "Test".to_string(),
            level: "must".to_string(),
            status: "enforced".to_string(),
            scope: vec![],
        }]);
        assert!(proxy.is_cache_valid());
    }

    #[test]
    fn mcp_proxy_clear_cache() {
        let mut proxy = McpProxy::new("https://design.example.com", "token-123");
        proxy.cache_revision("R42".to_string());
        proxy.clear_cache();
        assert!(!proxy.is_cache_valid());
    }

    #[test]
    fn design_overview_serializes() {
        let overview = DesignOverview {
            clauses: vec![ClauseSummary {
                id: "DC-12".to_string(),
                title: "Test".to_string(),
                level: "must".to_string(),
                status: "enforced".to_string(),
                scope: vec![],
            }],
            revision: "R42".to_string(),
        };
        let json = serde_json::to_string(&overview).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("R42"));
    }

    #[test]
    fn clause_detail_serializes() {
        let detail = ClauseDetail {
            id: "DC-12".to_string(),
            title: "Test".to_string(),
            level: "must".to_string(),
            status: "enforced".to_string(),
            statement: "MUST NOT".to_string(),
            verification: serde_json::json!({}),
            waivers: vec![],
        };
        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn check_finding_serializes() {
        let finding = CheckFinding {
            clause_id: "DC-12".to_string(),
            verdict: "violation".to_string(),
            file: "src/main.rs".to_string(),
            line: 42,
            message: "Violation".to_string(),
            confidence: 0.95,
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn amendment_draft_serializes() {
        let draft = AmendmentDraft {
            amendment_id: "A-31".to_string(),
            clause_id: Some("DC-12".to_string()),
            action: "edit".to_string(),
            redline_diff: serde_json::json!({}),
            status: "draft".to_string(),
            drafted_by: "agent".to_string(),
        };
        let json = serde_json::to_string(&draft).unwrap();
        assert!(json.contains("A-31"));
    }

    #[test]
    fn waiver_draft_serializes() {
        let draft = WaiverDraft {
            clause_id: "DC-12".to_string(),
            paths: vec!["src/main.rs".to_string()],
            reason: "Exception".to_string(),
            until: "2027-01-01".to_string(),
            status: "draft".to_string(),
        };
        let json = serde_json::to_string(&draft).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn mcp_tool_call_serializes() {
        let call = McpToolCall {
            tool_name: "design_clauses_for_paths".to_string(),
            arguments: serde_json::json!({"paths": ["src/main.rs"]}),
        };
        let json = serde_json::to_string(&call).unwrap();
        assert!(json.contains("design_clauses_for_paths"));
    }

    #[test]
    fn mcp_tool_result_serializes() {
        let result = McpToolResult {
            content: "OK".to_string(),
            is_error: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("OK"));
    }
}
