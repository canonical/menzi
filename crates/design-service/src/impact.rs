use crate::types::*;
use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub amendment_id: String,
    pub project_id: ProjectId,
    pub affected_files: Vec<AffectedFile>,
    pub violations: Vec<ImpactViolation>,
    pub effort_estimate: EffortEstimate,
    pub conflicts: Vec<ImpactConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedFile {
    pub path: String,
    pub change_type: String,
    pub clauses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactViolation {
    pub clause_id: String,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffortEstimate {
    pub story_points: i32,
    pub days: f64,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactConflict {
    pub clause_id: String,
    pub conflict_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceResult {
    pub revision_id: String,
    pub target_type: String,
    pub target_id: String,
    pub findings: Vec<ConformanceFinding>,
    pub passed: bool,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceFinding {
    pub clause_id: String,
    pub verdict: String,
    pub evidence: Vec<Evidence>,
    pub confidence: f64,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationPlan {
    pub clause_id: String,
    pub violations: Vec<ImplementationItem>,
    pub total_effort: EffortEstimate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationItem {
    pub file: String,
    pub description: String,
    pub effort: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_report_serializes() {
        let report = ImpactReport {
            amendment_id: "A-31".to_string(),
            project_id: ProjectId::new(),
            affected_files: vec![AffectedFile {
                path: "src/main.rs".to_string(),
                change_type: "modified".to_string(),
                clauses: vec!["DC-12".to_string()],
            }],
            violations: vec![],
            effort_estimate: EffortEstimate {
                story_points: 3,
                days: 1.5,
                confidence: "medium".to_string(),
            },
            conflicts: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("A-31"));
    }

    #[test]
    fn conformance_result_serializes() {
        let result = ConformanceResult {
            revision_id: "R42".to_string(),
            target_type: "pr".to_string(),
            target_id: "pr-412".to_string(),
            findings: vec![],
            passed: true,
            checked_at: "2026-09-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("pr-412"));
    }

    #[test]
    fn implementation_plan_serializes() {
        let plan = ImplementationPlan {
            clause_id: "DC-12".to_string(),
            violations: vec![ImplementationItem {
                file: "src/main.rs".to_string(),
                description: "Fix violation".to_string(),
                effort: "2 hours".to_string(),
            }],
            total_effort: EffortEstimate {
                story_points: 3,
                days: 1.5,
                confidence: "medium".to_string(),
            },
        };
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn effort_estimate_serializes() {
        let estimate = EffortEstimate {
            story_points: 5,
            days: 2.5,
            confidence: "high".to_string(),
        };
        let json = serde_json::to_string(&estimate).unwrap();
        assert!(json.contains("\"story_points\":5"));
    }

    #[test]
    fn impact_conflict_serializes() {
        let conflict = ImpactConflict {
            clause_id: "DC-12".to_string(),
            conflict_type: "logical".to_string(),
            description: "Conflicts with DC-15".to_string(),
        };
        let json = serde_json::to_string(&conflict).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn conformance_finding_serializes() {
        let finding = ConformanceFinding {
            clause_id: "DC-12".to_string(),
            verdict: "violation".to_string(),
            evidence: vec![],
            confidence: 0.95,
            suggestion: Some("Fix this".to_string()),
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn affected_file_serializes() {
        let file = AffectedFile {
            path: "src/main.rs".to_string(),
            change_type: "modified".to_string(),
            clauses: vec!["DC-12".to_string()],
        };
        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("src/main.rs"));
    }

    #[test]
    fn impact_violation_serializes() {
        let violation = ImpactViolation {
            clause_id: "DC-12".to_string(),
            severity: "high".to_string(),
            description: "New violation".to_string(),
        };
        let json = serde_json::to_string(&violation).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn implementation_item_serializes() {
        let item = ImplementationItem {
            file: "src/main.rs".to_string(),
            description: "Fix violation".to_string(),
            effort: "2 hours".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("src/main.rs"));
    }
}
