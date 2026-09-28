use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignOverviewTool {
    pub clauses: Vec<ToolClauseSummary>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolClauseSummary {
    pub id: String,
    pub title: String,
    pub level: String,
    pub status: String,
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClausesForPathsTool {
    pub clauses: Vec<ToolClauseDetail>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolClauseDetail {
    pub id: String,
    pub title: String,
    pub level: String,
    pub status: String,
    pub statement: String,
    pub verification: serde_json::Value,
    pub waivers: Vec<ToolWaiver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolWaiver {
    pub id: String,
    pub paths: Vec<String>,
    pub reason: String,
    pub until: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTool {
    pub results: Vec<SearchResult>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub clause_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckChangesTool {
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
pub struct ProposeAmendmentTool {
    pub amendment_id: String,
    pub clause_id: Option<String>,
    pub action: String,
    pub redline_diff: serde_json::Value,
    pub status: String,
    pub drafted_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposeWaiverTool {
    pub clause_id: String,
    pub paths: Vec<String>,
    pub reason: String,
    pub until: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAmendmentsTool {
    pub amendments: Vec<PendingAmendment>,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAmendment {
    pub amendment_id: String,
    pub clause_id: String,
    pub action: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct DesignMcpServer {
    pub design_service_url: String,
    pub session_token: String,
    pub cached_revision: Option<String>,
    pub cached_clauses: Vec<ToolClauseSummary>,
}

impl DesignMcpServer {
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

    pub fn cache_clauses(&mut self, clauses: Vec<ToolClauseSummary>) {
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
    fn mcp_server_stores_config() {
        let server = DesignMcpServer::new("https://design.example.com", "token-123");
        assert_eq!(server.design_service_url, "https://design.example.com");
        assert_eq!(server.session_token, "token-123");
    }

    #[test]
    fn mcp_server_caches_revision() {
        let mut server = DesignMcpServer::new("https://design.example.com", "token-123");
        server.cache_revision("R42".to_string());
        assert_eq!(server.get_cached_revision(), Some("R42"));
    }

    #[test]
    fn mcp_server_caches_clauses() {
        let mut server = DesignMcpServer::new("https://design.example.com", "token-123");
        server.cache_revision("R42".to_string());
        server.cache_clauses(vec![ToolClauseSummary {
            id: "DC-12".to_string(),
            title: "Test".to_string(),
            level: "must".to_string(),
            status: "enforced".to_string(),
            scope: vec![],
        }]);
        assert!(server.is_cache_valid());
    }

    #[test]
    fn mcp_server_clear_cache() {
        let mut server = DesignMcpServer::new("https://design.example.com", "token-123");
        server.cache_revision("R42".to_string());
        server.clear_cache();
        assert!(!server.is_cache_valid());
    }

    #[test]
    fn design_overview_tool_serializes() {
        let tool = DesignOverviewTool {
            clauses: vec![ToolClauseSummary {
                id: "DC-12".to_string(),
                title: "Test".to_string(),
                level: "must".to_string(),
                status: "enforced".to_string(),
                scope: vec![],
            }],
            revision: "R42".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("R42"));
    }

    #[test]
    fn clauses_for_paths_tool_serializes() {
        let tool = ClausesForPathsTool {
            clauses: vec![ToolClauseDetail {
                id: "DC-12".to_string(),
                title: "Test".to_string(),
                level: "must".to_string(),
                status: "enforced".to_string(),
                statement: "MUST NOT".to_string(),
                verification: serde_json::json!({}),
                waivers: vec![],
            }],
            revision: "R42".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn search_tool_serializes() {
        let tool = SearchTool {
            results: vec![SearchResult {
                clause_id: "DC-12".to_string(),
                title: "Test".to_string(),
                snippet: "MUST NOT".to_string(),
                score: 0.95,
            }],
            revision: "R42".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn check_changes_tool_serializes() {
        let tool = CheckChangesTool {
            findings: vec![CheckFinding {
                clause_id: "DC-12".to_string(),
                verdict: "violation".to_string(),
                file: "src/main.rs".to_string(),
                line: 42,
                message: "Violation".to_string(),
                confidence: 0.95,
            }],
            revision: "R42".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn propose_amendment_tool_serializes() {
        let tool = ProposeAmendmentTool {
            amendment_id: "A-31".to_string(),
            clause_id: Some("DC-12".to_string()),
            action: "edit".to_string(),
            redline_diff: serde_json::json!({}),
            status: "draft".to_string(),
            drafted_by: "agent".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("A-31"));
    }

    #[test]
    fn propose_waiver_tool_serializes() {
        let tool = ProposeWaiverTool {
            clause_id: "DC-12".to_string(),
            paths: vec!["src/main.rs".to_string()],
            reason: "Exception".to_string(),
            until: "2027-01-01".to_string(),
            status: "draft".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn pending_amendments_tool_serializes() {
        let tool = PendingAmendmentsTool {
            amendments: vec![PendingAmendment {
                amendment_id: "A-31".to_string(),
                clause_id: "DC-12".to_string(),
                action: "edit".to_string(),
                status: "open".to_string(),
            }],
            revision: "R42".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("A-31"));
    }
}
