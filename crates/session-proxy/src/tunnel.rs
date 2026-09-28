use menzi_common::ids::SessionId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConnection {
    pub session_id: SessionId,
    pub instance_name: String,
    pub status: TunnelStatus,
    pub connected_at: String,
    pub last_heartbeat: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelStatus {
    Connecting,
    Connected,
    Disconnected,
    Reconnecting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelMessage {
    pub message_type: MessageType,
    pub session_id: SessionId,
    pub payload: serde_json::Value,
    pub sequence: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Request,
    Response,
    Event,
    Heartbeat,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFanout {
    pub session_id: SessionId,
    pub viewers: Vec<String>,
    pub last_sequence: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptArchive {
    pub session_id: SessionId,
    pub opencode_session_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub events_count: i64,
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeRequest {
    pub session_id: SessionId,
    pub last_sequence: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResponse {
    pub session_id: SessionId,
    pub missed_events: Vec<TunnelMessage>,
    pub current_sequence: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TunnelManager {
    pub tunnels: HashMap<SessionId, TunnelConnection>,
    pub fanouts: HashMap<SessionId, EventFanout>,
    pub archives: HashMap<SessionId, TranscriptArchive>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnels: HashMap::new(),
            fanouts: HashMap::new(),
            archives: HashMap::new(),
        }
    }

    pub fn register_tunnel(&mut self, tunnel: TunnelConnection) {
        self.tunnels.insert(tunnel.session_id, tunnel);
    }

    pub fn disconnect_tunnel(&mut self, session_id: &SessionId) {
        if let Some(tunnel) = self.tunnels.get_mut(session_id) {
            tunnel.status = TunnelStatus::Disconnected;
        }
    }

    pub fn get_tunnel(&self, session_id: &SessionId) -> Option<&TunnelConnection> {
        self.tunnels.get(session_id)
    }

    pub fn add_viewer(&mut self, session_id: &SessionId, viewer_id: String) {
        let fanout = self.fanouts.entry(*session_id).or_insert(EventFanout {
            session_id: *session_id,
            viewers: vec![],
            last_sequence: 0,
        });
        if !fanout.viewers.contains(&viewer_id) {
            fanout.viewers.push(viewer_id);
        }
    }

    pub fn remove_viewer(&mut self, session_id: &SessionId, viewer_id: &str) {
        if let Some(fanout) = self.fanouts.get_mut(session_id) {
            fanout.viewers.retain(|v| v != viewer_id);
        }
    }

    pub fn get_viewer_count(&self, session_id: &SessionId) -> usize {
        self.fanouts
            .get(session_id)
            .map(|f| f.viewers.len())
            .unwrap_or(0)
    }

    pub fn archive_transcript(&mut self, archive: TranscriptArchive) {
        self.archives.insert(archive.session_id, archive);
    }

    pub fn get_archive(&self, session_id: &SessionId) -> Option<&TranscriptArchive> {
        self.archives.get(session_id)
    }

    pub fn list_active_tunnels(&self) -> Vec<&TunnelConnection> {
        self.tunnels
            .values()
            .filter(|t| matches!(t.status, TunnelStatus::Connected | TunnelStatus::Connecting))
            .collect()
    }

    pub fn update_heartbeat(&mut self, session_id: &SessionId, timestamp: String) {
        if let Some(tunnel) = self.tunnels.get_mut(session_id) {
            tunnel.last_heartbeat = timestamp;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_manager_registers_tunnel() {
        let mut manager = TunnelManager::new();
        let tunnel = TunnelConnection {
            session_id: SessionId::new(),
            instance_name: "instance-1".to_string(),
            status: TunnelStatus::Connected,
            connected_at: "2026-09-28T00:00:00Z".to_string(),
            last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
        };
        manager.register_tunnel(tunnel);
        assert!(!manager.tunnels.is_empty());
    }

    #[test]
    fn tunnel_manager_disconnects_tunnel() {
        let mut manager = TunnelManager::new();
        let session_id = SessionId::new();
        let tunnel = TunnelConnection {
            session_id,
            instance_name: "instance-1".to_string(),
            status: TunnelStatus::Connected,
            connected_at: "2026-09-28T00:00:00Z".to_string(),
            last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
        };
        manager.register_tunnel(tunnel);
        manager.disconnect_tunnel(&session_id);
        assert!(matches!(
            manager.get_tunnel(&session_id).unwrap().status,
            TunnelStatus::Disconnected
        ));
    }

    #[test]
    fn tunnel_manager_adds_viewers() {
        let mut manager = TunnelManager::new();
        let session_id = SessionId::new();
        manager.add_viewer(&session_id, "viewer-1".to_string());
        manager.add_viewer(&session_id, "viewer-2".to_string());
        assert_eq!(manager.get_viewer_count(&session_id), 2);
    }

    #[test]
    fn tunnel_manager_removes_viewers() {
        let mut manager = TunnelManager::new();
        let session_id = SessionId::new();
        manager.add_viewer(&session_id, "viewer-1".to_string());
        manager.add_viewer(&session_id, "viewer-2".to_string());
        manager.remove_viewer(&session_id, "viewer-1");
        assert_eq!(manager.get_viewer_count(&session_id), 1);
    }

    #[test]
    fn tunnel_manager_lists_active_tunnels() {
        let mut manager = TunnelManager::new();
        let tunnel = TunnelConnection {
            session_id: SessionId::new(),
            instance_name: "instance-1".to_string(),
            status: TunnelStatus::Connected,
            connected_at: "2026-09-28T00:00:00Z".to_string(),
            last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
        };
        manager.register_tunnel(tunnel);
        assert_eq!(manager.list_active_tunnels().len(), 1);
    }

    #[test]
    fn tunnel_message_serializes() {
        let msg = TunnelMessage {
            message_type: MessageType::Event,
            session_id: SessionId::new(),
            payload: serde_json::json!({"key": "value"}),
            sequence: 42,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"sequence\":42"));
    }

    #[test]
    fn tunnel_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&TunnelStatus::Connected).unwrap(),
            "\"connected\""
        );
        assert_eq!(
            serde_json::to_string(&TunnelStatus::Disconnected).unwrap(),
            "\"disconnected\""
        );
        assert_eq!(
            serde_json::to_string(&TunnelStatus::Connecting).unwrap(),
            "\"connecting\""
        );
        assert_eq!(
            serde_json::to_string(&TunnelStatus::Reconnecting).unwrap(),
            "\"reconnecting\""
        );
    }

    #[test]
    fn message_type_roundtrips() {
        assert_eq!(
            serde_json::to_string(&MessageType::Request).unwrap(),
            "\"request\""
        );
        assert_eq!(
            serde_json::to_string(&MessageType::Response).unwrap(),
            "\"response\""
        );
        assert_eq!(
            serde_json::to_string(&MessageType::Event).unwrap(),
            "\"event\""
        );
        assert_eq!(
            serde_json::to_string(&MessageType::Heartbeat).unwrap(),
            "\"heartbeat\""
        );
        assert_eq!(
            serde_json::to_string(&MessageType::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn transcript_archive_serializes() {
        let archive = TranscriptArchive {
            session_id: SessionId::new(),
            opencode_session_id: "oc-123".to_string(),
            started_at: "2026-09-28T00:00:00Z".to_string(),
            ended_at: None,
            events_count: 100,
            storage_path: "/archives/session-1.json".to_string(),
        };
        let json = serde_json::to_string(&archive).unwrap();
        assert!(json.contains("oc-123"));
    }

    #[test]
    fn resume_request_serializes() {
        let request = ResumeRequest {
            session_id: SessionId::new(),
            last_sequence: 42,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"last_sequence\":42"));
    }

    #[test]
    fn resume_response_serializes() {
        let response = ResumeResponse {
            session_id: SessionId::new(),
            missed_events: vec![],
            current_sequence: 100,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"current_sequence\":100"));
    }

    #[test]
    fn event_fanout_serializes() {
        let fanout = EventFanout {
            session_id: SessionId::new(),
            viewers: vec!["viewer-1".to_string()],
            last_sequence: 42,
        };
        let json = serde_json::to_string(&fanout).unwrap();
        assert!(json.contains("viewer-1"));
    }
}
