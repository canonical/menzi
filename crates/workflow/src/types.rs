use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub payload: serde_json::Value,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Prebuild,
    ConformanceSweep,
    #[serde(rename = "pr_design_review")]
    PRDesignReview,
    ImpactAnalysis,
    DriftDetection,
    PreviewBuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildJob {
    pub project_id: ProjectId,
    pub branch: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceSweepJob {
    pub project_id: ProjectId,
    pub revision_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRDesignReviewJob {
    pub project_id: ProjectId,
    pub pr_id: String,
    pub revision_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisJob {
    pub project_id: ProjectId,
    pub amendment_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectionJob {
    pub project_id: ProjectId,
    pub revision_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewBuildJob {
    pub project_id: ProjectId,
    pub preview_id: String,
    pub commit_sha: String,
    pub build_path: BuildPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildPath {
    Fast,
    Reproducible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: String,
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_type_roundtrips() {
        assert_eq!(
            serde_json::to_string(&JobType::Prebuild).unwrap(),
            "\"prebuild\""
        );
        assert_eq!(
            serde_json::to_string(&JobType::ConformanceSweep).unwrap(),
            "\"conformance_sweep\""
        );
        assert_eq!(
            serde_json::to_string(&JobType::PRDesignReview).unwrap(),
            "\"pr_design_review\""
        );
        assert_eq!(
            serde_json::to_string(&JobType::ImpactAnalysis).unwrap(),
            "\"impact_analysis\""
        );
        assert_eq!(
            serde_json::to_string(&JobType::DriftDetection).unwrap(),
            "\"drift_detection\""
        );
        assert_eq!(
            serde_json::to_string(&JobType::PreviewBuild).unwrap(),
            "\"preview_build\""
        );
    }

    #[test]
    fn job_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&JobStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn build_path_roundtrips() {
        assert_eq!(serde_json::to_string(&BuildPath::Fast).unwrap(), "\"fast\"");
        assert_eq!(
            serde_json::to_string(&BuildPath::Reproducible).unwrap(),
            "\"reproducible\""
        );
    }

    #[test]
    fn job_serializes() {
        let job = Job {
            id: "job-1".to_string(),
            job_type: JobType::Prebuild,
            status: JobStatus::Pending,
            payload: serde_json::json!({"project_id": "proj-1"}),
            created_at: "2026-09-28T00:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
            error: None,
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("\"job_type\":\"prebuild\""));
        assert!(json.contains("\"status\":\"pending\""));
    }

    #[test]
    fn prebuild_job_serializes() {
        let job = PrebuildJob {
            project_id: ProjectId::new(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("\"branch\":\"main\""));
    }

    #[test]
    fn conformance_sweep_job_serializes() {
        let job = ConformanceSweepJob {
            project_id: ProjectId::new(),
            revision_id: "rev-42".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("rev-42"));
    }

    #[test]
    fn pr_design_review_job_serializes() {
        let job = PRDesignReviewJob {
            project_id: ProjectId::new(),
            pr_id: "pr-412".to_string(),
            revision_id: "rev-42".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("pr-412"));
    }

    #[test]
    fn impact_analysis_job_serializes() {
        let job = ImpactAnalysisJob {
            project_id: ProjectId::new(),
            amendment_id: "A-31".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("A-31"));
    }

    #[test]
    fn drift_detection_job_serializes() {
        let job = DriftDetectionJob {
            project_id: ProjectId::new(),
            revision_id: "rev-42".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("rev-42"));
    }

    #[test]
    fn preview_build_job_serializes() {
        let job = PreviewBuildJob {
            project_id: ProjectId::new(),
            preview_id: "preview-1".to_string(),
            commit_sha: "abc123".to_string(),
            build_path: BuildPath::Fast,
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("preview-1"));
        assert!(json.contains("\"build_path\":\"fast\""));
    }

    #[test]
    fn job_result_serializes() {
        let result = JobResult {
            job_id: "job-1".to_string(),
            success: true,
            output: serde_json::json!({"findings": 0}),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
