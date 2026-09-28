use crate::types::*;
use menzi_common::ids::UserId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceUpdate {
    pub user_id: UserId,
    pub web_push_enabled: Option<bool>,
    pub email_enabled: Option<bool>,
    pub chat_enabled: Option<bool>,
    pub muted_types: Option<Vec<NotificationType>>,
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuietHours {
    pub start: String,
    pub end: String,
    pub timezone: String,
}

impl QuietHours {
    pub fn is_quiet_now(&self, current_time: &str) -> bool {
        if self.start <= self.end {
            current_time >= self.start.as_str() && current_time <= self.end.as_str()
        } else {
            current_time >= self.start.as_str() || current_time <= self.end.as_str()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationHistory {
    pub notifications: Vec<Notification>,
    pub total_count: i32,
    pub unread_count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_update_serializes() {
        let update = PreferenceUpdate {
            user_id: UserId::new(),
            web_push_enabled: Some(false),
            email_enabled: None,
            chat_enabled: None,
            muted_types: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"web_push_enabled\":false"));
    }

    #[test]
    fn quiet_hours_checks_time() {
        let quiet = QuietHours {
            start: "22:00".to_string(),
            end: "08:00".to_string(),
            timezone: "UTC".to_string(),
        };
        assert!(quiet.is_quiet_now("23:00"));
        assert!(!quiet.is_quiet_now("12:00"));
    }

    #[test]
    fn notification_history_serializes() {
        let history = NotificationHistory {
            notifications: vec![],
            total_count: 0,
            unread_count: 0,
        };
        let json = serde_json::to_string(&history).unwrap();
        assert!(json.contains("\"total_count\":0"));
    }

    #[test]
    fn quiet_hours_serializes() {
        let quiet = QuietHours {
            start: "22:00".to_string(),
            end: "08:00".to_string(),
            timezone: "UTC".to_string(),
        };
        let json = serde_json::to_string(&quiet).unwrap();
        assert!(json.contains("22:00"));
    }
}
