use menzi_common::ids::SessionId;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FanoutManager {
    sessions: HashMap<SessionId, Vec<FanoutViewer>>,
}

#[derive(Debug, Clone)]
pub struct FanoutViewer {
    pub viewer_id: String,
    pub last_sequence: i64,
}

impl FanoutManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, session_id: SessionId, viewer_id: String) {
        let viewers = self.sessions.entry(session_id).or_default();
        if !viewers.iter().any(|v| v.viewer_id == viewer_id) {
            viewers.push(FanoutViewer {
                viewer_id,
                last_sequence: 0,
            });
        }
    }

    pub fn unsubscribe(&mut self, session_id: &SessionId, viewer_id: &str) {
        if let Some(viewers) = self.sessions.get_mut(session_id) {
            viewers.retain(|v| v.viewer_id != viewer_id);
        }
    }

    pub fn get_subscribers(&self, session_id: &SessionId) -> Vec<&FanoutViewer> {
        self.sessions
            .get(session_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn update_sequence(&mut self, session_id: &SessionId, viewer_id: &str, sequence: i64) {
        if let Some(viewers) = self.sessions.get_mut(session_id) {
            for viewer in viewers.iter_mut() {
                if viewer.viewer_id == viewer_id {
                    viewer.last_sequence = sequence;
                }
            }
        }
    }

    pub fn get_active_sessions(&self) -> Vec<SessionId> {
        self.sessions
            .iter()
            .filter(|(_, viewers)| !viewers.is_empty())
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Default for FanoutManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fanout_manager_subscribes_viewer() {
        let mut manager = FanoutManager::new();
        manager.subscribe(SessionId::new(), "viewer-1".to_string());
        assert_eq!(manager.get_active_sessions().len(), 1);
    }

    #[test]
    fn fanout_manager_unsubscribes_viewer() {
        let mut manager = FanoutManager::new();
        let session_id = SessionId::new();
        manager.subscribe(session_id, "viewer-1".to_string());
        manager.unsubscribe(&session_id, "viewer-1");
        assert_eq!(manager.get_active_sessions().len(), 0);
    }

    #[test]
    fn fanout_manager_updates_sequence() {
        let mut manager = FanoutManager::new();
        let session_id = SessionId::new();
        manager.subscribe(session_id, "viewer-1".to_string());
        manager.update_sequence(&session_id, "viewer-1", 42);
        let subscribers = manager.get_subscribers(&session_id);
        assert_eq!(subscribers[0].last_sequence, 42);
    }

    #[test]
    fn fanout_manager_tracks_active_sessions() {
        let mut manager = FanoutManager::new();
        manager.subscribe(SessionId::new(), "viewer-1".to_string());
        manager.subscribe(SessionId::new(), "viewer-2".to_string());
        assert_eq!(manager.get_active_sessions().len(), 2);
    }
}
