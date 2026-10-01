use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use menzi_common::ids::{ProjectId, UserId};
use serde_json::json;

pub const HEADER_PROJECT_ID: &str = "x-menzi-project-id";
pub const HEADER_USER_ID: &str = "x-menzi-user-id";
pub const HEADER_SESSION_ID: &str = "x-menzi-session-id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantContext {
    pub project_id: Option<ProjectId>,
    pub user_id: Option<UserId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantError(pub String);

impl std::fmt::Display for TenantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

impl TenantContext {
    pub fn from_headers(headers: &HeaderMap) -> Result<Self, TenantError> {
        let project_id = match header(headers, HEADER_PROJECT_ID) {
            Some(raw) => Some(
                raw.parse()
                    .map_err(|_| TenantError(format!("invalid project id '{raw}'")))?,
            ),
            None => None,
        };

        let user_id = match header(headers, HEADER_USER_ID) {
            Some(raw) => Some(
                raw.parse()
                    .map_err(|_| TenantError(format!("invalid user id '{raw}'")))?,
            ),
            None => None,
        };

        if project_id.is_none() && user_id.is_none() {
            return Err(TenantError(
                "missing x-menzi-project-id or x-menzi-user-id header".to_string(),
            ));
        }

        Ok(Self {
            project_id,
            user_id,
        })
    }

    pub fn scope_key(&self) -> String {
        if let Some(project_id) = self.project_id {
            return format!("project:{project_id}");
        }
        match self.user_id {
            Some(user_id) => format!("user:{user_id}"),
            None => Self::default_scope(),
        }
    }

    pub fn default_scope() -> String {
        "default".to_string()
    }
}

pub struct TenantExtractor(pub TenantContext);

impl FromRequestParts<()> for TenantExtractor {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &()) -> Result<Self, Self::Rejection> {
        match TenantContext::from_headers(&parts.headers) {
            Ok(tenant) => Ok(Self(tenant)),
            Err(error) => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": error.to_string()})),
            )
                .into_response()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn from_headers_parses_user_only() {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_USER_ID, UserId::new().to_string().parse().unwrap());
        let tenant = TenantContext::from_headers(&headers).unwrap();
        assert!(tenant.project_id.is_none());
        assert!(tenant.user_id.is_some());
    }

    #[test]
    fn from_headers_parses_full_tenant() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_PROJECT_ID,
            ProjectId::new().to_string().parse().unwrap(),
        );
        headers.insert(HEADER_USER_ID, UserId::new().to_string().parse().unwrap());
        let tenant = TenantContext::from_headers(&headers).unwrap();
        assert!(tenant.project_id.is_some());
        assert!(tenant.user_id.is_some());
    }

    #[test]
    fn from_headers_rejects_an_empty_tenant() {
        let headers = HeaderMap::new();
        let error = TenantContext::from_headers(&headers).unwrap_err();
        assert!(error.0.contains("x-menzi-project-id"));
    }

    #[test]
    fn from_headers_rejects_an_invalid_project() {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_PROJECT_ID, "not-a-uuid".parse().unwrap());
        assert!(TenantContext::from_headers(&headers).is_err());
    }

    #[test]
    fn scope_key_uses_project_when_present() {
        let project_id = ProjectId::new();
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_PROJECT_ID, project_id.to_string().parse().unwrap());
        let tenant = TenantContext::from_headers(&headers).unwrap();
        assert_eq!(tenant.scope_key(), format!("project:{project_id}"));
    }

    #[test]
    fn scope_key_uses_user_without_project() {
        let user_id = UserId::new();
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_USER_ID, user_id.to_string().parse().unwrap());
        let tenant = TenantContext::from_headers(&headers).unwrap();
        assert_eq!(tenant.scope_key(), format!("user:{user_id}"));
    }

    #[tokio::test]
    async fn extractor_accepts_valid_tenant() {
        let app = Router::new().route(
            "/tenant",
            get(|TenantExtractor(tenant): TenantExtractor| async move {
                Json(json!({"scope": tenant.scope_key()}))
            }),
        );
        let user_id = UserId::new();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/tenant")
                    .header(HEADER_USER_ID, user_id.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extractor_rejects_an_empty_tenant() {
        let app = Router::new().route(
            "/tenant",
            get(|TenantExtractor(_): TenantExtractor| async { StatusCode::OK }),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/tenant")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn extractor_returns_scope_in_response() {
        let app = Router::new().route(
            "/tenant",
            get(|TenantExtractor(tenant): TenantExtractor| async move {
                Json(json!({"scope": tenant.scope_key()}))
            }),
        );
        let project_id = ProjectId::new();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/tenant")
                    .header(HEADER_PROJECT_ID, project_id.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["scope"], format!("project:{project_id}"));
    }
}
