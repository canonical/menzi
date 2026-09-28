use chrono::{DateTime, Utc};
use menzi_common::ids::EventId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event<T> {
    pub id: EventId,
    pub event_type: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub timestamp: DateTime<Utc>,
    pub payload: T,
}

impl<T> Event<T> {
    pub fn new(
        event_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            id: EventId::new(),
            event_type: event_type.into(),
            aggregate_id: aggregate_id.into(),
            aggregate_type: aggregate_type.into(),
            timestamp: Utc::now(),
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_new_creates_with_unique_ids() {
        let e1 = Event::new("test.event", "agg-1", "test", 42);
        let e2 = Event::new("test.event", "agg-1", "test", 42);
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn event_new_sets_fields() {
        let e = Event::new("session.ready", "sess-1", "session", "payload-data");
        assert_eq!(e.event_type, "session.ready");
        assert_eq!(e.aggregate_id, "sess-1");
        assert_eq!(e.aggregate_type, "session");
        assert_eq!(e.payload, "payload-data");
    }

    #[test]
    fn event_serializes_to_json() {
        let e = Event::new("test.event", "agg-1", "test", 42);
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"event_type\":\"test.event\""));
        assert!(json.contains("\"aggregate_id\":\"agg-1\""));
    }

    #[test]
    fn event_deserializes_from_json() {
        let e = Event::new("test.event", "agg-1", "test", 42);
        let json = serde_json::to_string(&e).unwrap();
        let deserialized: Event<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, "test.event");
        assert_eq!(deserialized.payload, 42);
    }
}
