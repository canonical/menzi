use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewStatus {
    Pending,
    Ready,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    pub id: String,
    pub project_id: ProjectId,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub status: PreviewStatus,
    pub mode: String,
    pub url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewSpec {
    pub project_id: ProjectId,
    pub source_instance: String,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub mode: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&PreviewStatus::Ready).unwrap(),
            "\"ready\""
        );
        assert_eq!(
            serde_json::to_string(&PreviewStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::from_str::<PreviewStatus>("\"stopped\"").unwrap(),
            PreviewStatus::Stopped
        );
    }

    #[test]
    fn preview_serializes_with_expected_shape() {
        let preview = Preview {
            id: "prv-1".to_string(),
            project_id: ProjectId::new(),
            commit_sha: Some("abc1234".to_string()),
            branch: Some("feature/x".to_string()),
            status: PreviewStatus::Ready,
            mode: "pinned".to_string(),
            url: "http://prv-1.preview.dev.local".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&preview).unwrap();
        assert_eq!(json["id"], "prv-1");
        assert_eq!(json["status"], "ready");
        assert_eq!(json["mode"], "pinned");
        assert!(json.get("commit_sha").is_some());
        assert!(json.get("branch").is_some());
    }
}
