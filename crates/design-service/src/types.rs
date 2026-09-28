use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignRevision {
    pub id: String,
    pub project_id: ProjectId,
    pub revision_number: i32,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignClause {
    pub id: String,
    pub project_id: ProjectId,
    pub clause_id: String,
    pub version: i32,
    pub title: String,
    pub status: ClauseStatus,
    pub level: ClauseLevel,
    pub visibility: ClauseVisibility,
    pub scope: ClauseScope,
    pub approvers: Vec<String>,
    pub statement: String,
    pub rationale_ref: Option<String>,
    pub verification: Verification,
    pub supersedes: Option<String>,
    pub trial_until: Option<String>,
    pub revision_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClauseStatus {
    Ratified,
    Implementing,
    Enforced,
    Deprecated,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClauseLevel {
    Must,
    Should,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClauseVisibility {
    Project,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClauseScope {
    pub paths: Vec<String>,
    pub components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub checks: Vec<Check>,
    pub review: Option<Review>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    #[serde(rename = "type")]
    pub check_type: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub rubric: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignAmendment {
    pub id: String,
    pub project_id: ProjectId,
    pub amendment_id: String,
    pub clause_id: Option<String>,
    pub action: AmendmentAction,
    pub status: AmendmentStatus,
    pub redline_diff: Option<serde_json::Value>,
    pub rationale: Option<String>,
    pub impact_report: Option<serde_json::Value>,
    pub approvals: Vec<Approval>,
    pub created_by: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendmentAction {
    Add,
    Edit,
    Deprecate,
    Supersede,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendmentStatus {
    Draft,
    Open,
    InReview,
    Approved,
    Ratified,
    Rejected,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub user_id: String,
    pub approved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignWaiver {
    pub id: String,
    pub project_id: ProjectId,
    pub clause_id: String,
    pub paths: Vec<String>,
    pub reason: String,
    pub until: String,
    pub granted_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignBaseline {
    pub id: String,
    pub project_id: ProjectId,
    pub name: String,
    pub revision_id: String,
    pub branch_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub id: String,
    pub project_id: ProjectId,
    pub revision_id: String,
    pub target_type: String,
    pub target_id: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub clause_id: String,
    pub verdict: Verdict,
    pub evidence: Vec<Evidence>,
    pub confidence: f64,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Violation,
    Unclear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub file: String,
    pub line: i32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignDiscussion {
    pub id: String,
    pub project_id: ProjectId,
    pub title: String,
    pub status: DiscussionStatus,
    pub created_by: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscussionStatus {
    Open,
    Proposed,
    Abandoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_revision_serializes() {
        let revision = DesignRevision {
            id: "rev-1".to_string(),
            project_id: ProjectId::new(),
            revision_number: 1,
            created_by: "user-1".to_string(),
            created_at: "2026-09-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&revision).unwrap();
        assert!(json.contains("\"revision_number\":1"));
    }

    #[test]
    fn clause_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&ClauseStatus::Ratified).unwrap(),
            "\"ratified\""
        );
        assert_eq!(
            serde_json::to_string(&ClauseStatus::Implementing).unwrap(),
            "\"implementing\""
        );
        assert_eq!(
            serde_json::to_string(&ClauseStatus::Enforced).unwrap(),
            "\"enforced\""
        );
        assert_eq!(
            serde_json::to_string(&ClauseStatus::Deprecated).unwrap(),
            "\"deprecated\""
        );
        assert_eq!(
            serde_json::to_string(&ClauseStatus::Superseded).unwrap(),
            "\"superseded\""
        );
    }

    #[test]
    fn clause_level_roundtrips() {
        assert_eq!(
            serde_json::to_string(&ClauseLevel::Must).unwrap(),
            "\"must\""
        );
        assert_eq!(
            serde_json::to_string(&ClauseLevel::Should).unwrap(),
            "\"should\""
        );
    }

    #[test]
    fn clause_visibility_roundtrips() {
        assert_eq!(
            serde_json::to_string(&ClauseVisibility::Project).unwrap(),
            "\"project\""
        );
        assert_eq!(
            serde_json::to_string(&ClauseVisibility::Private).unwrap(),
            "\"private\""
        );
    }

    #[test]
    fn amendment_action_roundtrips() {
        assert_eq!(
            serde_json::to_string(&AmendmentAction::Add).unwrap(),
            "\"add\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentAction::Edit).unwrap(),
            "\"edit\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentAction::Deprecate).unwrap(),
            "\"deprecate\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentAction::Supersede).unwrap(),
            "\"supersede\""
        );
    }

    #[test]
    fn amendment_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::Draft).unwrap(),
            "\"draft\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::Open).unwrap(),
            "\"open\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::InReview).unwrap(),
            "\"in_review\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::Approved).unwrap(),
            "\"approved\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::Ratified).unwrap(),
            "\"ratified\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::Rejected).unwrap(),
            "\"rejected\""
        );
        assert_eq!(
            serde_json::to_string(&AmendmentStatus::Withdrawn).unwrap(),
            "\"withdrawn\""
        );
    }

    #[test]
    fn verdict_roundtrips() {
        assert_eq!(serde_json::to_string(&Verdict::Pass).unwrap(), "\"pass\"");
        assert_eq!(
            serde_json::to_string(&Verdict::Violation).unwrap(),
            "\"violation\""
        );
        assert_eq!(
            serde_json::to_string(&Verdict::Unclear).unwrap(),
            "\"unclear\""
        );
    }

    #[test]
    fn discussion_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&DiscussionStatus::Open).unwrap(),
            "\"open\""
        );
        assert_eq!(
            serde_json::to_string(&DiscussionStatus::Proposed).unwrap(),
            "\"proposed\""
        );
        assert_eq!(
            serde_json::to_string(&DiscussionStatus::Abandoned).unwrap(),
            "\"abandoned\""
        );
    }

    #[test]
    fn clause_scope_serializes() {
        let scope = ClauseScope {
            paths: vec!["cmd/cli/**".to_string()],
            components: vec!["cli".to_string()],
        };
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("cmd/cli/**"));
        assert!(json.contains("cli"));
    }

    #[test]
    fn verification_serializes() {
        let verification = Verification {
            checks: vec![Check {
                check_type: "import-rule".to_string(),
                config: serde_json::json!({"deny": {"from": "cmd/cli/**", "to": "internal/store/**"}}),
            }],
            review: Some(Review {
                rubric:
                    "Any data access from CLI code paths not via internal/client is a violation."
                        .to_string(),
            }),
        };
        let json = serde_json::to_string(&verification).unwrap();
        assert!(json.contains("import-rule"));
        assert!(json.contains("rubric"));
    }

    #[test]
    fn finding_serializes() {
        let finding = Finding {
            clause_id: "DC-12".to_string(),
            verdict: Verdict::Violation,
            evidence: vec![Evidence {
                file: "cmd/cli/list.go".to_string(),
                line: 42,
                snippet: "db.Query(...)".to_string(),
            }],
            confidence: 0.95,
            suggestion: Some("Use internal/client instead".to_string()),
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("violation"));
        assert!(json.contains("cmd/cli/list.go"));
    }

    #[test]
    fn conformance_report_serializes() {
        let report = ConformanceReport {
            id: "report-1".to_string(),
            project_id: ProjectId::new(),
            revision_id: "rev-1".to_string(),
            target_type: "pr".to_string(),
            target_id: "pr-412".to_string(),
            findings: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("pr-412"));
    }

    #[test]
    fn design_clause_serializes() {
        let clause = DesignClause {
            id: "clause-1".to_string(),
            project_id: ProjectId::new(),
            clause_id: "DC-12".to_string(),
            version: 3,
            title: "CLI talks to the daemon only via its API".to_string(),
            status: ClauseStatus::Enforced,
            level: ClauseLevel::Must,
            visibility: ClauseVisibility::Project,
            scope: ClauseScope {
                paths: vec!["cmd/cli/**".to_string()],
                components: vec!["cli".to_string()],
            },
            approvers: vec!["@api-team".to_string()],
            statement: "The CLI MUST NOT access the database directly.".to_string(),
            rationale_ref: Some("A-12".to_string()),
            verification: Verification {
                checks: vec![],
                review: None,
            },
            supersedes: None,
            trial_until: None,
            revision_id: "rev-1".to_string(),
        };
        let json = serde_json::to_string(&clause).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("\"status\":\"enforced\""));
        assert!(json.contains("\"level\":\"must\""));
    }

    #[test]
    fn design_amendment_serializes() {
        let amendment = DesignAmendment {
            id: "amendment-1".to_string(),
            project_id: ProjectId::new(),
            amendment_id: "A-31".to_string(),
            clause_id: Some("DC-12".to_string()),
            action: AmendmentAction::Edit,
            status: AmendmentStatus::InReview,
            redline_diff: None,
            rationale: Some("Migrations run before the daemon exists".to_string()),
            impact_report: None,
            approvals: vec![],
            created_by: "user-1".to_string(),
        };
        let json = serde_json::to_string(&amendment).unwrap();
        assert!(json.contains("A-31"));
        assert!(json.contains("\"status\":\"in_review\""));
    }

    #[test]
    fn design_waiver_serializes() {
        let waiver = DesignWaiver {
            id: "waiver-1".to_string(),
            project_id: ProjectId::new(),
            clause_id: "DC-12".to_string(),
            paths: vec!["cmd/cli/migrate.go".to_string()],
            reason: "Migrations run before the daemon exists".to_string(),
            until: "2027-03-01".to_string(),
            granted_by: "user-1".to_string(),
        };
        let json = serde_json::to_string(&waiver).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("cmd/cli/migrate.go"));
    }

    #[test]
    fn design_baseline_serializes() {
        let baseline = DesignBaseline {
            id: "baseline-1".to_string(),
            project_id: ProjectId::new(),
            name: "v2-baseline".to_string(),
            revision_id: "rev-31".to_string(),
            branch_patterns: vec!["release/2.*".to_string()],
        };
        let json = serde_json::to_string(&baseline).unwrap();
        assert!(json.contains("v2-baseline"));
        assert!(json.contains("release/2.*"));
    }

    #[test]
    fn design_discussion_serializes() {
        let discussion = DesignDiscussion {
            id: "discussion-1".to_string(),
            project_id: ProjectId::new(),
            title: "Should we add a new API endpoint?".to_string(),
            status: DiscussionStatus::Open,
            created_by: "user-1".to_string(),
        };
        let json = serde_json::to_string(&discussion).unwrap();
        assert!(json.contains("Should we add a new API endpoint?"));
        assert!(json.contains("\"status\":\"open\""));
    }

    #[test]
    fn approval_serializes() {
        let approval = Approval {
            user_id: "user-1".to_string(),
            approved_at: "2026-09-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&approval).unwrap();
        assert!(json.contains("user-1"));
    }

    #[test]
    fn evidence_serializes() {
        let evidence = Evidence {
            file: "src/main.rs".to_string(),
            line: 10,
            snippet: "let x = 1;".to_string(),
        };
        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("\"line\":10"));
    }
}
