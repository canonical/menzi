use ulid::Ulid;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EventId(Ulid);

impl EventId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct OrgId(Uuid);

impl OrgId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for OrgId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OrgId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for OrgId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectId(Uuid);

impl ProjectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ProjectId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct UserId(Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_new_generates_unique_ids() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn session_id_from_uuid_roundtrips() {
        let uuid = Uuid::new_v4();
        let id = SessionId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn session_id_display_formats_as_uuid() {
        let uuid = Uuid::new_v4();
        let id = SessionId::from_uuid(uuid);
        assert_eq!(format!("{}", id), uuid.to_string());
    }

    #[test]
    fn session_id_from_str_parses_uuid() {
        let uuid = Uuid::new_v4();
        let parsed: SessionId = uuid.to_string().parse().unwrap();
        assert_eq!(parsed.as_uuid(), &uuid);
    }

    #[test]
    fn session_id_from_str_rejects_invalid() {
        let parsed: std::result::Result<SessionId, _> = "invalid".parse();
        assert!(parsed.is_err());
    }

    #[test]
    fn event_id_new_generates_unique_ids() {
        let id1 = EventId::new();
        let id2 = EventId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn org_id_new_generates_unique_ids() {
        let id1 = OrgId::new();
        let id2 = OrgId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn project_id_new_generates_unique_ids() {
        let id1 = ProjectId::new();
        let id2 = ProjectId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn user_id_new_generates_unique_ids() {
        let id1 = UserId::new();
        let id2 = UserId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn org_id_from_str_roundtrips() {
        let uuid = Uuid::new_v4();
        let id = OrgId::from_uuid(uuid);
        let parsed: OrgId = id.to_string().parse().unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn project_id_from_str_roundtrips() {
        let uuid = Uuid::new_v4();
        let id = ProjectId::from_uuid(uuid);
        let parsed: ProjectId = id.to_string().parse().unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn user_id_from_str_roundtrips() {
        let uuid = Uuid::new_v4();
        let id = UserId::from_uuid(uuid);
        let parsed: UserId = id.to_string().parse().unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn id_from_str_rejects_invalid() {
        assert!("not-a-uuid".parse::<OrgId>().is_err());
        assert!("not-a-uuid".parse::<ProjectId>().is_err());
        assert!("not-a-uuid".parse::<UserId>().is_err());
    }
}
