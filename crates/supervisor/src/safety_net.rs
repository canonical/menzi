use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyNetConfig {
    pub idle_threshold_secs: u64,
    pub check_on_idle: bool,
    pub post_findings: bool,
}

impl Default for SafetyNetConfig {
    fn default() -> Self {
        Self {
            idle_threshold_secs: 300,
            check_on_idle: true,
            post_findings: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyNetResult {
    pub session_id: SessionId,
    pub triggered: bool,
    pub findings: Vec<SafetyFinding>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyFinding {
    pub check_type: String,
    pub severity: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleCheck {
    pub session_id: SessionId,
    pub idle_secs: u64,
    pub has_changes: bool,
}

impl IdleCheck {
    pub fn should_trigger(&self, threshold: u64) -> bool {
        self.idle_secs >= threshold && self.has_changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_net_config_defaults() {
        let config = SafetyNetConfig::default();
        assert_eq!(config.idle_threshold_secs, 300);
        assert!(config.check_on_idle);
        assert!(config.post_findings);
    }

    #[test]
    fn idle_check_triggers_when_idle_with_changes() {
        let check = IdleCheck {
            session_id: SessionId::new(),
            idle_secs: 300,
            has_changes: true,
        };
        assert!(check.should_trigger(300));
    }

    #[test]
    fn idle_check_does_not_trigger_when_active() {
        let check = IdleCheck {
            session_id: SessionId::new(),
            idle_secs: 100,
            has_changes: true,
        };
        assert!(!check.should_trigger(300));
    }

    #[test]
    fn idle_check_does_not_trigger_without_changes() {
        let check = IdleCheck {
            session_id: SessionId::new(),
            idle_secs: 300,
            has_changes: false,
        };
        assert!(!check.should_trigger(300));
    }

    #[test]
    fn safety_net_result_serializes() {
        let result = SafetyNetResult {
            session_id: SessionId::new(),
            triggered: true,
            findings: vec![SafetyFinding {
                check_type: "design".to_string(),
                severity: "warning".to_string(),
                message: "Violation".to_string(),
                file: Some("src/main.rs".to_string()),
                line: Some(42),
            }],
            message: "Check triggered".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("design"));
    }
}
