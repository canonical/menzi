use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub notification: Notification,
    pub channels: Vec<Channel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    pub notification_id: String,
    pub channel_results: Vec<ChannelResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResult {
    pub channel: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPushMessage {
    pub endpoint: String,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub channel: String,
    pub text: String,
    pub blocks: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct NotificationDispatcher {
    pub vapid_key: String,
    pub email_from: String,
}

impl NotificationDispatcher {
    pub fn new(vapid_key: impl Into<String>, email_from: impl Into<String>) -> Self {
        Self {
            vapid_key: vapid_key.into(),
            email_from: email_from.into(),
        }
    }

    pub fn should_dispatch(
        &self,
        prefs: &NotificationPreferences,
        notification_type: NotificationType,
        channel: Channel,
    ) -> bool {
        if prefs.muted_types.contains(&notification_type) {
            return false;
        }
        match channel {
            Channel::WebPush => prefs.web_push_enabled,
            Channel::Email => prefs.email_enabled,
            Channel::Chat => prefs.chat_enabled,
        }
    }

    pub fn create_email_message(&self, notification: &Notification) -> EmailMessage {
        EmailMessage {
            to: "user@example.com".to_string(),
            subject: notification.title.clone(),
            body_html: format!("<p>{}</p>", notification.body),
            body_text: notification.body.clone(),
        }
    }

    pub fn create_web_push_message(&self, notification: &Notification) -> WebPushMessage {
        WebPushMessage {
            endpoint: "https://push.example.com/123".to_string(),
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: None,
            badge: None,
            data: notification
                .action_url
                .as_ref()
                .map(|url| serde_json::json!({"url": url})),
        }
    }

    pub fn create_chat_message(&self, notification: &Notification) -> ChatMessage {
        ChatMessage {
            channel: "general".to_string(),
            text: format!("{}: {}", notification.title, notification.body),
            blocks: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatcher_creates() {
        let dispatcher = NotificationDispatcher::new("vapid-key", "noreply@example.com");
        assert_eq!(dispatcher.vapid_key, "vapid-key");
    }

    #[test]
    fn dispatcher_checks_preferences() {
        let dispatcher = NotificationDispatcher::new("vapid-key", "noreply@example.com");
        let prefs = NotificationPreferences {
            user_id: menzi_common::ids::UserId::new(),
            web_push_enabled: true,
            email_enabled: false,
            chat_enabled: true,
            muted_types: vec![],
            quiet_hours_start: None,
            quiet_hours_end: None,
        };
        assert!(dispatcher.should_dispatch(
            &prefs,
            NotificationType::PreviewReady,
            Channel::WebPush
        ));
        assert!(!dispatcher.should_dispatch(
            &prefs,
            NotificationType::PreviewReady,
            Channel::Email
        ));
        assert!(dispatcher.should_dispatch(&prefs, NotificationType::PreviewReady, Channel::Chat));
    }

    #[test]
    fn dispatcher_blocks_muted_types() {
        let dispatcher = NotificationDispatcher::new("vapid-key", "noreply@example.com");
        let prefs = NotificationPreferences {
            user_id: menzi_common::ids::UserId::new(),
            web_push_enabled: true,
            email_enabled: true,
            chat_enabled: true,
            muted_types: vec![NotificationType::PreviewReady],
            quiet_hours_start: None,
            quiet_hours_end: None,
        };
        assert!(!dispatcher.should_dispatch(
            &prefs,
            NotificationType::PreviewReady,
            Channel::WebPush
        ));
    }

    #[test]
    fn dispatcher_creates_email_message() {
        let dispatcher = NotificationDispatcher::new("vapid-key", "noreply@example.com");
        let notification = Notification {
            id: "notif-1".to_string(),
            user_id: menzi_common::ids::UserId::new(),
            title: "Preview ready".to_string(),
            body: "Your preview is ready".to_string(),
            notification_type: NotificationType::PreviewReady,
            priority: NotificationPriority::Normal,
            read: false,
            created_at: "2026-09-28T00:00:00Z".to_string(),
            action_url: None,
        };
        let email = dispatcher.create_email_message(&notification);
        assert_eq!(email.subject, "Preview ready");
    }

    #[test]
    fn dispatcher_creates_web_push_message() {
        let dispatcher = NotificationDispatcher::new("vapid-key", "noreply@example.com");
        let notification = Notification {
            id: "notif-1".to_string(),
            user_id: menzi_common::ids::UserId::new(),
            title: "Preview ready".to_string(),
            body: "Your preview is ready".to_string(),
            notification_type: NotificationType::PreviewReady,
            priority: NotificationPriority::Normal,
            read: false,
            created_at: "2026-09-28T00:00:00Z".to_string(),
            action_url: Some("https://preview.example.com/1".to_string()),
        };
        let push = dispatcher.create_web_push_message(&notification);
        assert_eq!(push.title, "Preview ready");
    }

    #[test]
    fn dispatcher_creates_chat_message() {
        let dispatcher = NotificationDispatcher::new("vapid-key", "noreply@example.com");
        let notification = Notification {
            id: "notif-1".to_string(),
            user_id: menzi_common::ids::UserId::new(),
            title: "Preview ready".to_string(),
            body: "Your preview is ready".to_string(),
            notification_type: NotificationType::PreviewReady,
            priority: NotificationPriority::Normal,
            read: false,
            created_at: "2026-09-28T00:00:00Z".to_string(),
            action_url: None,
        };
        let chat = dispatcher.create_chat_message(&notification);
        assert!(chat.text.contains("Preview ready"));
    }

    #[test]
    fn dispatch_request_serializes() {
        let request = DispatchRequest {
            notification: Notification {
                id: "notif-1".to_string(),
                user_id: menzi_common::ids::UserId::new(),
                title: "Test".to_string(),
                body: "Test body".to_string(),
                notification_type: NotificationType::PreviewReady,
                priority: NotificationPriority::Normal,
                read: false,
                created_at: "2026-09-28T00:00:00Z".to_string(),
                action_url: None,
            },
            channels: vec![Channel::WebPush],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Test"));
    }

    #[test]
    fn dispatch_result_serializes() {
        let result = DispatchResult {
            notification_id: "notif-1".to_string(),
            channel_results: vec![ChannelResult {
                channel: "web_push".to_string(),
                success: true,
                error: None,
            }],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("notif-1"));
    }

    #[test]
    fn email_message_serializes() {
        let message = EmailMessage {
            to: "user@example.com".to_string(),
            subject: "Test".to_string(),
            body_html: "<p>Test</p>".to_string(),
            body_text: "Test".to_string(),
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("user@example.com"));
    }

    #[test]
    fn web_push_message_serializes() {
        let message = WebPushMessage {
            endpoint: "https://push.example.com/123".to_string(),
            title: "Test".to_string(),
            body: "Test body".to_string(),
            icon: None,
            badge: None,
            data: None,
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("https://push.example.com/123"));
    }

    #[test]
    fn chat_message_serializes() {
        let message = ChatMessage {
            channel: "general".to_string(),
            text: "Test".to_string(),
            blocks: None,
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("Test"));
    }

    #[test]
    fn channel_result_serializes() {
        let result = ChannelResult {
            channel: "web_push".to_string(),
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
