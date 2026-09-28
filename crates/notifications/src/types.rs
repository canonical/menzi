use menzi_common::ids::UserId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub user_id: UserId,
    pub title: String,
    pub body: String,
    pub notification_type: NotificationType,
    pub priority: NotificationPriority,
    pub read: bool,
    pub created_at: String,
    pub action_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    PreviewReady,
    PreviewFailed,
    PreviewCommented,
    ApprovalNeeded,
    ClauseRatified,
    PermissionRequest,
    SessionIdle,
    SystemAlert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub user_id: UserId,
    pub web_push_enabled: bool,
    pub email_enabled: bool,
    pub chat_enabled: bool,
    pub muted_types: Vec<NotificationType>,
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
}

impl NotificationPreferences {
    pub fn should_notify(&self, notification_type: NotificationType, channel: Channel) -> bool {
        if self.muted_types.contains(&notification_type) {
            return false;
        }
        match channel {
            Channel::WebPush => self.web_push_enabled,
            Channel::Email => self.email_enabled,
            Channel::Chat => self.chat_enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    WebPush,
    Email,
    Chat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPushSubscription {
    pub user_id: UserId,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationBatch {
    pub notifications: Vec<Notification>,
    pub total_count: i32,
    pub unread_count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_type_roundtrips() {
        assert_eq!(
            serde_json::to_string(&NotificationType::PreviewReady).unwrap(),
            "\"preview_ready\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::PreviewFailed).unwrap(),
            "\"preview_failed\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::ApprovalNeeded).unwrap(),
            "\"approval_needed\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::ClauseRatified).unwrap(),
            "\"clause_ratified\""
        );
    }

    #[test]
    fn notification_priority_roundtrips() {
        assert_eq!(
            serde_json::to_string(&NotificationPriority::Low).unwrap(),
            "\"low\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationPriority::Normal).unwrap(),
            "\"normal\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationPriority::High).unwrap(),
            "\"high\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationPriority::Critical).unwrap(),
            "\"critical\""
        );
    }

    #[test]
    fn notification_preferences_allows_when_enabled() {
        let prefs = NotificationPreferences {
            user_id: UserId::new(),
            web_push_enabled: true,
            email_enabled: true,
            chat_enabled: false,
            muted_types: vec![],
            quiet_hours_start: None,
            quiet_hours_end: None,
        };
        assert!(prefs.should_notify(NotificationType::PreviewReady, Channel::WebPush));
        assert!(prefs.should_notify(NotificationType::PreviewReady, Channel::Email));
        assert!(!prefs.should_notify(NotificationType::PreviewReady, Channel::Chat));
    }

    #[test]
    fn notification_preferences_blocks_muted_types() {
        let prefs = NotificationPreferences {
            user_id: UserId::new(),
            web_push_enabled: true,
            email_enabled: true,
            chat_enabled: true,
            muted_types: vec![NotificationType::PreviewReady],
            quiet_hours_start: None,
            quiet_hours_end: None,
        };
        assert!(!prefs.should_notify(NotificationType::PreviewReady, Channel::WebPush));
        assert!(!prefs.should_notify(NotificationType::PreviewReady, Channel::Email));
        assert!(!prefs.should_notify(NotificationType::PreviewReady, Channel::Chat));
    }

    #[test]
    fn notification_serializes() {
        let notification = Notification {
            id: "notif-1".to_string(),
            user_id: UserId::new(),
            title: "Preview ready".to_string(),
            body: "Your preview is ready".to_string(),
            notification_type: NotificationType::PreviewReady,
            priority: NotificationPriority::Normal,
            read: false,
            created_at: "2026-09-28T00:00:00Z".to_string(),
            action_url: Some("https://preview.example.com/p/1".to_string()),
        };
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("Preview ready"));
        assert!(json.contains("\"notification_type\":\"preview_ready\""));
    }

    #[test]
    fn web_push_subscription_serializes() {
        let sub = WebPushSubscription {
            user_id: UserId::new(),
            endpoint: "https://push.example.com/123".to_string(),
            p256dh: "p256dh-key".to_string(),
            auth: "auth-token".to_string(),
        };
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("https://push.example.com/123"));
    }

    #[test]
    fn notification_batch_serializes() {
        let batch = NotificationBatch {
            notifications: vec![],
            total_count: 0,
            unread_count: 0,
        };
        let json = serde_json::to_string(&batch).unwrap();
        assert!(json.contains("\"total_count\":0"));
    }

    #[test]
    fn notification_preferences_serializes() {
        let prefs = NotificationPreferences {
            user_id: UserId::new(),
            web_push_enabled: true,
            email_enabled: false,
            chat_enabled: true,
            muted_types: vec![NotificationType::PreviewFailed],
            quiet_hours_start: Some("22:00".to_string()),
            quiet_hours_end: Some("08:00".to_string()),
        };
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("\"web_push_enabled\":true"));
        assert!(json.contains("quiet_hours_start"));
    }
}
