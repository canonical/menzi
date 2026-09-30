use async_trait::async_trait;
use menzi_common::ids::ProjectId;
use menzi_common::{MenziError, Result};
use std::collections::HashSet;
use std::sync::Mutex;

use crate::types::WorkspaceKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    User(menzi_common::ids::UserId),
    Service(String),
}

impl Principal {
    pub fn is_service(&self) -> bool {
        matches!(self, Principal::Service(_))
    }

    pub fn user_id(&self) -> Option<menzi_common::ids::UserId> {
        match self {
            Principal::User(id) => Some(*id),
            Principal::Service(_) => None,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Principal::User(id) => id.to_string(),
            Principal::Service(name) => name.clone(),
        }
    }
}

#[async_trait]
pub trait Authorizer: Send + Sync {
    async fn may_use_workspace(&self, principal: &Principal, key: &WorkspaceKey) -> Result<bool>;
}

pub struct PermissiveAuthorizer;

#[async_trait]
impl Authorizer for PermissiveAuthorizer {
    async fn may_use_workspace(&self, _principal: &Principal, _key: &WorkspaceKey) -> Result<bool> {
        Ok(true)
    }
}

pub struct MembershipAuthorizer {
    allowed: Mutex<HashSet<ProjectId>>,
    service_names: HashSet<String>,
}

impl MembershipAuthorizer {
    pub fn new() -> Self {
        Self {
            allowed: Mutex::new(HashSet::new()),
            service_names: HashSet::new(),
        }
    }

    pub fn with_project(self, project_id: ProjectId) -> Self {
        self.allowed
            .lock()
            .expect("membership lock")
            .insert(project_id);
        self
    }

    pub fn with_service(mut self, name: impl Into<String>) -> Self {
        self.service_names.insert(name.into());
        self
    }

    pub fn allow(&self, project_id: ProjectId) {
        self.allowed
            .lock()
            .expect("membership lock")
            .insert(project_id);
    }
}

impl Default for MembershipAuthorizer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Authorizer for MembershipAuthorizer {
    async fn may_use_workspace(&self, principal: &Principal, key: &WorkspaceKey) -> Result<bool> {
        if let Principal::Service(name) = principal {
            return Ok(self.service_names.contains(name));
        }
        Ok(self
            .allowed
            .lock()
            .expect("membership lock")
            .contains(&key.project_id))
    }
}

pub const SERVICE_AUTONOMOUS: &str = "autonomous";

pub fn header_value(headers: &axum::http::HeaderMap) -> Option<Principal> {
    let service = headers.get("x-menzi-service")?.to_str().ok()?;
    if service.is_empty() {
        return None;
    }
    Some(Principal::Service(service.to_string()))
}

pub fn require_service(principal: &Principal) -> Result<()> {
    match principal {
        Principal::Service(_) => Ok(()),
        Principal::User(_) => Err(MenziError::Forbidden(
            "this operation is for a service principal".to_string(),
        )),
    }
}

pub fn require_user(principal: &Principal) -> Result<menzi_common::ids::UserId> {
    match principal {
        Principal::User(id) => Ok(*id),
        Principal::Service(name) => Err(MenziError::Forbidden(format!(
            "service {name} cannot act as a user"
        ))),
    }
}

pub async fn authorize(
    authorizer: &dyn Authorizer,
    principal: &Principal,
    key: &WorkspaceKey,
) -> Result<()> {
    if authorizer.may_use_workspace(principal, key).await? {
        Ok(())
    } else {
        Err(MenziError::Forbidden(format!(
            "{} may not use the workspace for project {}",
            principal.name(),
            key.project_id
        )))
    }
}

pub async fn authorize_project(
    authorizer: &dyn Authorizer,
    principal: &Principal,
    project_id: ProjectId,
) -> Result<()> {
    if authorizer
        .may_use_workspace(
            principal,
            &WorkspaceKey::new(menzi_common::ids::UserId::new(), project_id),
        )
        .await?
    {
        Ok(())
    } else {
        Err(MenziError::Forbidden(format!(
            "{} may not use project {project_id}",
            principal.name()
        )))
    }
}

pub async fn principal_from_headers(headers: &axum::http::HeaderMap) -> Result<Principal> {
    if let Some(service) = header_value(headers) {
        return Ok(service);
    }
    match headers
        .get("x-menzi-user-id")
        .and_then(|value| value.to_str().ok())
    {
        Some(raw) => raw
            .parse()
            .map(Principal::User)
            .map_err(|_| MenziError::Validation(format!("invalid user id '{raw}'"))),
        None => Err(MenziError::Unauthorized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use menzi_common::ids::UserId;

    #[tokio::test]
    async fn the_permissive_authorizer_allows_everything() {
        let authorizer = PermissiveAuthorizer;
        let key = WorkspaceKey::new(UserId::new(), ProjectId::new());
        assert!(authorizer
            .may_use_workspace(&Principal::User(UserId::new()), &key)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn membership_allows_a_user_in_the_project() {
        let project = ProjectId::new();
        let authorizer = MembershipAuthorizer::new().with_project(project);
        let key = WorkspaceKey::new(UserId::new(), project);
        assert!(authorizer
            .may_use_workspace(&Principal::User(UserId::new()), &key)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn membership_rejects_a_user_outside_the_project() {
        let authorizer = MembershipAuthorizer::new().with_project(ProjectId::new());
        let key = WorkspaceKey::new(UserId::new(), ProjectId::new());
        assert!(!authorizer
            .may_use_workspace(&Principal::User(UserId::new()), &key)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn membership_rejects_an_unknown_service() {
        let authorizer = MembershipAuthorizer::new().with_service("autonomous");
        let key = WorkspaceKey::new(UserId::new(), ProjectId::new());
        assert!(!authorizer
            .may_use_workspace(&Principal::Service("rogue".to_string()), &key)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn a_named_service_is_allowed_anywhere() {
        let authorizer = MembershipAuthorizer::new().with_service(SERVICE_AUTONOMOUS);
        let key = WorkspaceKey::new(UserId::new(), ProjectId::new());
        assert!(authorizer
            .may_use_workspace(&Principal::Service(SERVICE_AUTONOMOUS.to_string()), &key)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn a_project_can_be_allowed_after_construction() {
        let project = ProjectId::new();
        let authorizer = MembershipAuthorizer::new();
        let key = WorkspaceKey::new(UserId::new(), project);
        assert!(!authorizer
            .may_use_workspace(&Principal::User(UserId::new()), &key)
            .await
            .unwrap());
        authorizer.allow(project);
        assert!(authorizer
            .may_use_workspace(&Principal::User(UserId::new()), &key)
            .await
            .unwrap());
    }

    #[test]
    fn a_user_cannot_act_as_a_service() {
        assert!(require_service(&Principal::User(UserId::new())).is_err());
        assert!(require_service(&Principal::Service("autonomous".to_string())).is_ok());
    }

    #[test]
    fn a_service_cannot_act_as_a_user() {
        assert!(require_user(&Principal::Service("autonomous".to_string())).is_err());
        let user = UserId::new();
        assert_eq!(require_user(&Principal::User(user)).unwrap(), user);
    }

    #[test]
    fn the_service_header_is_read_when_present() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(header_value(&headers).is_none());
        headers.insert("x-menzi-service", "autonomous".parse().unwrap());
        assert_eq!(
            header_value(&headers),
            Some(Principal::Service("autonomous".to_string()))
        );
    }

    #[test]
    fn an_empty_service_header_is_ignored() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-menzi-service", "".parse().unwrap());
        assert!(header_value(&headers).is_none());
    }

    #[tokio::test]
    async fn authorize_reports_forbidden_for_an_outsider() {
        let authorizer = MembershipAuthorizer::new();
        let key = WorkspaceKey::new(UserId::new(), ProjectId::new());
        let error = authorize(&authorizer, &Principal::User(UserId::new()), &key)
            .await
            .unwrap_err();
        assert!(matches!(error, MenziError::Forbidden(_)));
    }
}
