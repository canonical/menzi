use crate::types::*;
use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildResult {
    pub project_id: ProjectId,
    pub image_name: String,
    pub baseline_snapshot: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceSweepResult {
    pub project_id: ProjectId,
    pub revision_id: String,
    pub total_clauses: i32,
    pub violations_found: i32,
    pub findings: Vec<SweepFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepFinding {
    pub clause_id: String,
    pub verdict: String,
    pub file: String,
    pub line: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRReviewResult {
    pub project_id: ProjectId,
    pub pr_id: String,
    pub revision_id: String,
    pub relevant_clauses: Vec<String>,
    pub findings: Vec<ReviewFinding>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub clause_id: String,
    pub verdict: String,
    pub file: String,
    pub line: i32,
    pub message: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactResult {
    pub project_id: ProjectId,
    pub amendment_id: String,
    pub affected_files: Vec<String>,
    pub violations: i32,
    pub effort_days: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResult {
    pub project_id: ProjectId,
    pub revision_id: String,
    pub drift_detected: bool,
    pub drifted_clauses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewBuildResult {
    pub project_id: ProjectId,
    pub preview_id: String,
    pub success: bool,
    pub url: String,
    pub build_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct JobHandler;

impl JobHandler {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_prebuild(&self, job: &PrebuildJob) -> PrebuildResult {
        PrebuildResult {
            project_id: job.project_id,
            image_name: format!("menzi-{}-{}", job.project_id, job.commit_sha),
            baseline_snapshot: format!("{}-baseline", job.branch),
            success: true,
        }
    }

    pub fn handle_conformance_sweep(&self, job: &ConformanceSweepJob) -> ConformanceSweepResult {
        ConformanceSweepResult {
            project_id: job.project_id,
            revision_id: job.revision_id.clone(),
            total_clauses: 0,
            violations_found: 0,
            findings: vec![],
        }
    }

    pub fn handle_pr_review(&self, job: &PRDesignReviewJob) -> PRReviewResult {
        PRReviewResult {
            project_id: job.project_id,
            pr_id: job.pr_id.clone(),
            revision_id: job.revision_id.clone(),
            relevant_clauses: vec![],
            findings: vec![],
            status: "pass".to_string(),
        }
    }

    pub fn handle_impact(&self, job: &ImpactAnalysisJob) -> ImpactResult {
        ImpactResult {
            project_id: job.project_id,
            amendment_id: job.amendment_id.clone(),
            affected_files: vec![],
            violations: 0,
            effort_days: 0.0,
        }
    }

    pub fn handle_drift(&self, job: &DriftDetectionJob) -> DriftResult {
        DriftResult {
            project_id: job.project_id,
            revision_id: job.revision_id.clone(),
            drift_detected: false,
            drifted_clauses: vec![],
        }
    }

    pub fn handle_preview_build(&self, job: &PreviewBuildJob) -> PreviewBuildResult {
        PreviewBuildResult {
            project_id: job.project_id,
            preview_id: job.preview_id.clone(),
            success: true,
            url: format!("https://preview.example.com/{}", job.preview_id),
            build_path: match job.build_path {
                BuildPath::Fast => "fast".to_string(),
                BuildPath::Reproducible => "reproducible".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_handler_handles_prebuild() {
        let handler = JobHandler::new();
        let job = PrebuildJob {
            project_id: ProjectId::new(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
        };
        let result = handler.handle_prebuild(&job);
        assert!(result.success);
    }

    #[test]
    fn job_handler_handles_conformance_sweep() {
        let handler = JobHandler::new();
        let job = ConformanceSweepJob {
            project_id: ProjectId::new(),
            revision_id: "R42".to_string(),
        };
        let result = handler.handle_conformance_sweep(&job);
        assert_eq!(result.revision_id, "R42");
    }

    #[test]
    fn job_handler_handles_pr_review() {
        let handler = JobHandler::new();
        let job = PRDesignReviewJob {
            project_id: ProjectId::new(),
            pr_id: "pr-412".to_string(),
            revision_id: "R42".to_string(),
        };
        let result = handler.handle_pr_review(&job);
        assert_eq!(result.pr_id, "pr-412");
    }

    #[test]
    fn job_handler_handles_impact() {
        let handler = JobHandler::new();
        let job = ImpactAnalysisJob {
            project_id: ProjectId::new(),
            amendment_id: "A-31".to_string(),
        };
        let result = handler.handle_impact(&job);
        assert_eq!(result.amendment_id, "A-31");
    }

    #[test]
    fn job_handler_handles_drift() {
        let handler = JobHandler::new();
        let job = DriftDetectionJob {
            project_id: ProjectId::new(),
            revision_id: "R42".to_string(),
        };
        let result = handler.handle_drift(&job);
        assert!(!result.drift_detected);
    }

    #[test]
    fn job_handler_handles_preview_build() {
        let handler = JobHandler::new();
        let job = PreviewBuildJob {
            project_id: ProjectId::new(),
            preview_id: "preview-1".to_string(),
            commit_sha: "abc123".to_string(),
            build_path: BuildPath::Fast,
        };
        let result = handler.handle_preview_build(&job);
        assert!(result.success);
    }

    #[test]
    fn prebuild_result_serializes() {
        let result = PrebuildResult {
            project_id: ProjectId::new(),
            image_name: "menzi-proj-1-abc123".to_string(),
            baseline_snapshot: "main-baseline".to_string(),
            success: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("menzi-proj-1-abc123"));
    }

    #[test]
    fn conformance_sweep_result_serializes() {
        let result = ConformanceSweepResult {
            project_id: ProjectId::new(),
            revision_id: "R42".to_string(),
            total_clauses: 12,
            violations_found: 3,
            findings: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"total_clauses\":12"));
    }

    #[test]
    fn pr_review_result_serializes() {
        let result = PRReviewResult {
            project_id: ProjectId::new(),
            pr_id: "pr-412".to_string(),
            revision_id: "R42".to_string(),
            relevant_clauses: vec!["DC-12".to_string()],
            findings: vec![],
            status: "pass".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("pr-412"));
    }

    #[test]
    fn impact_result_serializes() {
        let result = ImpactResult {
            project_id: ProjectId::new(),
            amendment_id: "A-31".to_string(),
            affected_files: vec!["src/main.rs".to_string()],
            violations: 2,
            effort_days: 1.5,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("A-31"));
    }

    #[test]
    fn drift_result_serializes() {
        let result = DriftResult {
            project_id: ProjectId::new(),
            revision_id: "R42".to_string(),
            drift_detected: true,
            drifted_clauses: vec!["DC-12".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn preview_build_result_serializes() {
        let result = PreviewBuildResult {
            project_id: ProjectId::new(),
            preview_id: "preview-1".to_string(),
            success: true,
            url: "https://preview.example.com/preview-1".to_string(),
            build_path: "fast".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("preview-1"));
    }

    #[test]
    fn sweep_finding_serializes() {
        let finding = SweepFinding {
            clause_id: "DC-12".to_string(),
            verdict: "violation".to_string(),
            file: "src/main.rs".to_string(),
            line: 42,
            message: "Violation".to_string(),
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("DC-12"));
    }

    #[test]
    fn review_finding_serializes() {
        let finding = ReviewFinding {
            clause_id: "DC-12".to_string(),
            verdict: "pass".to_string(),
            file: "src/main.rs".to_string(),
            line: 42,
            message: "OK".to_string(),
            confidence: 0.95,
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("DC-12"));
    }
}
