use crate::tunnel::{TranscriptArchive, TunnelMessage};
use menzi_common::ids::SessionId;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TranscriptArchiver {
    pub storage_path: String,
    pub sessions: HashMap<SessionId, Vec<TunnelMessage>>,
}

impl TranscriptArchiver {
    pub fn new(storage_path: impl Into<String>) -> Self {
        Self {
            storage_path: storage_path.into(),
            sessions: HashMap::new(),
        }
    }

    pub fn record_event(&mut self, session_id: SessionId, message: TunnelMessage) {
        let events = self.sessions.entry(session_id).or_default();
        events.push(message);
    }

    pub fn get_events(&self, session_id: &SessionId) -> Option<&Vec<TunnelMessage>> {
        self.sessions.get(session_id)
    }

    pub fn archive_session(&mut self, session_id: &SessionId) -> Option<TranscriptArchive> {
        let events = self.sessions.remove(session_id)?;
        Some(TranscriptArchive {
            session_id: *session_id,
            opencode_session_id: "oc-123".to_string(),
            started_at: "2026-09-28T00:00:00Z".to_string(),
            ended_at: Some("2026-09-28T01:00:00Z".to_string()),
            events_count: events.len() as i64,
            storage_path: format!("{}/{}.json", self.storage_path, session_id),
        })
    }

    pub fn get_event_count(&self, session_id: &SessionId) -> usize {
        self.sessions.get(session_id).map(|e| e.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::MessageType;

    #[test]
    fn transcript_archiver_records_events() {
        let mut archiver = TranscriptArchiver::new("/tmp/archives");
        let session_id = SessionId::new();
        let msg = TunnelMessage {
            message_type: MessageType::Event,
            session_id,
            payload: serde_json::json!({}),
            sequence: 1,
        };
        archiver.record_event(session_id, msg);
        assert_eq!(archiver.get_event_count(&session_id), 1);
    }

    #[test]
    fn transcript_archiver_archives_session() {
        let mut archiver = TranscriptArchiver::new("/tmp/archives");
        let session_id = SessionId::new();
        let msg = TunnelMessage {
            message_type: MessageType::Event,
            session_id,
            payload: serde_json::json!({}),
            sequence: 1,
        };
        archiver.record_event(session_id, msg);
        let archive = archiver.archive_session(&session_id).unwrap();
        assert_eq!(archive.events_count, 1);
    }

    #[test]
    fn transcript_archiver_returns_none_for_unknown_session() {
        let mut archiver = TranscriptArchiver::new("/tmp/archives");
        let archive = archiver.archive_session(&SessionId::new());
        assert!(archive.is_none());
    }
}
