use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use menzi_common::ids::{ProjectId, UserId};
use menzi_common::MenziError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::identity::{
    authorize, authorize_project, principal_from_headers, require_service, Principal,
};
use crate::manager::WorkspaceManager;
use crate::registry::SessionKind;
use crate::types::{TerminalSpec, WorkspaceKey, WorkspaceSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsureWorkspaceRequest {
    pub project_id: ProjectId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub user_id: Option<UserId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub kind: Option<SessionKind>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Clone)]
pub struct WorkspaceApiState {
    pub manager: Arc<WorkspaceManager>,
    pub source_instance: String,
    pub authorizer: Arc<dyn crate::identity::Authorizer>,
}

impl WorkspaceApiState {
    pub fn new(manager: Arc<WorkspaceManager>, source_instance: impl Into<String>) -> Self {
        Self {
            manager,
            source_instance: source_instance.into(),
            authorizer: Arc::new(crate::identity::PermissiveAuthorizer),
        }
    }

    pub fn with_authorizer(mut self, authorizer: Arc<dyn crate::identity::Authorizer>) -> Self {
        self.authorizer = authorizer;
        self
    }
}

pub struct Caller(pub Principal);

impl<S> FromRequestParts<S> for Caller
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match principal_from_headers(&parts.headers).await {
            Ok(principal) => Ok(Caller(principal)),
            Err(error) => Err(server_error(error)),
        }
    }
}

pub fn create_router(state: WorkspaceApiState) -> axum::Router {
    axum::Router::new()
        .route("/api/v1/workspaces", post(ensure_workspace))
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}",
            get(get_workspace)
                .delete(destroy_workspace)
                .post(suspend_workspace),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/start",
            post(start_workspace),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/connect",
            post(connect_workspace),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/sessions",
            get(list_sessions).post(open_session),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/prompt",
            post(run_prompt),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/interrupt",
            post(interrupt_session),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/terminal",
            post(run_terminal),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/diff",
            get(workspace_diff),
        )
        .route(
            "/api/v1/projects/{project_id}/workspaces",
            get(list_workspaces),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/sessions/{session_id}/workspace",
            get(session_workspace),
        )
        .route(
            "/api/v1/sessions/{session_id}/workspace",
            get(session_route),
        )
        .with_state(state)
}

fn server_error(error: MenziError) -> Response {
    let status = match error {
        MenziError::NotFound(_) => StatusCode::NOT_FOUND,
        MenziError::Unauthorized => StatusCode::UNAUTHORIZED,
        MenziError::Forbidden(_) => StatusCode::FORBIDDEN,
        MenziError::Conflict(_) => StatusCode::CONFLICT,
        MenziError::Validation(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(json!({ "error": error.to_string() }))).into_response()
}

fn server_result<T: Serialize>(result: menzi_common::Result<T>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => server_error(error),
    }
}

async fn guard(
    state: &WorkspaceApiState,
    caller: &Principal,
    key: &WorkspaceKey,
) -> Result<(), MenziError> {
    resolve_user(caller, Some(key.user_id))?;
    authorize(state.authorizer.as_ref(), caller, key).await
}

fn resolve_user(caller: &Principal, requested: Option<UserId>) -> Result<UserId, MenziError> {
    match (caller, requested) {
        (Principal::User(id), None) => Ok(*id),
        (Principal::User(id), Some(requested)) if *id == requested => Ok(requested),
        (Principal::User(_), Some(_)) => Err(MenziError::Forbidden(
            "a user may only act on their own workspace".to_string(),
        )),
        (Principal::Service(_), Some(requested)) => Ok(requested),
        (Principal::Service(name), None) => Err(MenziError::Validation(format!(
            "service {name} must say which user the workspace is for"
        ))),
    }
}

async fn ensure_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Json(request): Json<EnsureWorkspaceRequest>,
) -> Response {
    let user_id = match resolve_user(&caller, request.user_id) {
        Ok(user_id) => user_id,
        Err(error) => return server_error(error),
    };
    let key = WorkspaceKey::new(user_id, request.project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    let spec = WorkspaceSpec {
        user_id,
        project_id: request.project_id,
        source_instance: state.source_instance.clone(),
        name: request.name,
        branch: request.branch,
        commit_sha: request.commit_sha,
    };
    server_result(state.manager.ensure_workspace(&caller.name(), spec).await)
}

async fn get_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    server_result(state.manager.get_workspace(&key).await)
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionWorkspace {
    pub opencode_session: String,
    pub workspace_id: menzi_common::ids::WorkspaceId,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
    pub kind: SessionKind,
}

async fn session_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id, session_id)): Path<(UserId, ProjectId, String)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    match state.manager.session_binding(&session_id).await {
        Ok(Some(binding)) => {
            let workspace = match state
                .manager
                .get_workspace_by_id(binding.workspace_id)
                .await
            {
                Ok(workspace) => workspace,
                Err(error) => return server_error(error),
            };
            if workspace.user_id != user_id || workspace.project_id != project_id {
                return server_error(MenziError::Forbidden(
                    "that session runs in another workspace".to_string(),
                ));
            }
            Json(SessionWorkspace {
                opencode_session: binding.opencode_session,
                workspace_id: binding.workspace_id,
                endpoint: binding.endpoint,
                instance_name: binding.instance_name,
                kind: binding.kind,
            })
            .into_response()
        }
        Ok(None) => server_error(MenziError::NotFound(format!("session {session_id}"))),
        Err(error) => server_error(error),
    }
}

/// The session proxy asks this to route a session it holds no user for, so a
/// service principal is accepted here. The binding record carries the owner, and
/// a user is refused unless it is them.
async fn session_route(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path(session_id): Path<String>,
) -> Response {
    let binding = match state.manager.session_binding(&session_id).await {
        Ok(Some(binding)) => binding,
        Ok(None) => return server_error(MenziError::NotFound(format!("session {session_id}"))),
        Err(error) => return server_error(error),
    };
    if let Some(user_id) = caller.user_id() {
        if binding.user_id != user_id {
            return server_error(MenziError::Forbidden(
                "that session runs in another user's workspace".to_string(),
            ));
        }
    } else if let Err(error) = require_service(&caller) {
        return server_error(error);
    }
    Json(SessionWorkspace {
        opencode_session: binding.opencode_session,
        workspace_id: binding.workspace_id,
        endpoint: binding.endpoint,
        instance_name: binding.instance_name,
        kind: binding.kind,
    })
    .into_response()
}

async fn list_workspaces(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path(project_id): Path<ProjectId>,
) -> Response {
    if let Err(error) = authorize_project(state.authorizer.as_ref(), &caller, project_id).await {
        return server_error(error);
    }
    server_result(state.manager.list_workspaces(project_id).await)
}

async fn connect_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    match state.manager.connect(&key).await {
        Ok(endpoint) => Json(json!({ "endpoint": endpoint })).into_response(),
        Err(error) => server_error(error),
    }
}

async fn start_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    server_result(state.manager.start(&key).await)
}

async fn suspend_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    server_result(state.manager.suspend_workspace(&key).await)
}

async fn open_session(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
    Json(request): Json<OpenSessionRequest>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    match state
        .manager
        .open_session(&caller.name(), &key, request.title.as_deref())
        .await
    {
        Ok(session) => (StatusCode::CREATED, Json(session)).into_response(),
        Err(error) => server_error(error),
    }
}

async fn list_sessions(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    server_result(state.manager.list_sessions(&key).await)
}

async fn run_prompt(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
    Json(request): Json<PromptRequest>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    let kind = request.kind.unwrap_or(SessionKind::Interactive);
    let session_id = match request.session_id.clone() {
        Some(session_id) => session_id,
        None => match state
            .manager
            .open_session(&caller.name(), &key, request.title.as_deref())
            .await
        {
            Ok(session) => session.id,
            Err(error) => return server_error(error),
        },
    };
    server_result(
        state
            .manager
            .prompt_as(&caller.name(), &key, &session_id, &request.text, kind)
            .await,
    )
}

async fn interrupt_session(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    let sessions = match state.manager.list_sessions(&key).await {
        Ok(sessions) => sessions,
        Err(error) => return server_error(error),
    };
    if let Some(session) = sessions.first() {
        if let Err(error) = state.manager.interrupt(&key, &session.id).await {
            return server_error(error);
        }
    }
    Json(json!({ "interrupted": true })).into_response()
}

async fn run_terminal(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
    Json(spec): Json<TerminalSpec>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    server_result(state.manager.terminal(&key, &spec.into()).await)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffQuery {
    pub directory: Option<String>,
    pub path: Option<String>,
    pub version: Option<String>,
}

async fn workspace_diff(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
    Query(query): Query<DiffQuery>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    server_result(
        state
            .manager
            .tree_changes(
                &key,
                query.directory.as_deref(),
                query.path.as_deref(),
                query.version.as_deref(),
            )
            .await,
    )
}

async fn destroy_workspace(
    State(state): State<WorkspaceApiState>,
    Caller(caller): Caller,
    Path((user_id, project_id)): Path<(UserId, ProjectId)>,
) -> Response {
    let key = WorkspaceKey::new(user_id, project_id);
    if let Err(error) = guard(&state, &caller, &key).await {
        return server_error(error);
    }
    match state.manager.destroy_workspace(&caller.name(), &key).await {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(error) => server_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::testbed::{harness, head_cmd, numstat_all_cmd, status_cmd, Harness};
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn open_app(project: ProjectId) -> axum::Router {
        open_app_with_harness(project).0
    }

    fn open_app_with_harness(project: ProjectId) -> (axum::Router, Harness) {
        let h = harness();
        let api = WorkspaceApiState::new(Arc::new(h.manager.clone()), "mz-workspace")
            .with_authorizer(Arc::new(
                crate::identity::MembershipAuthorizer::new().with_project(project),
            ));
        (create_router(api), h)
    }

    fn caller_headers(user: UserId) -> Vec<(&'static str, String)> {
        vec![("x-menzi-user-id", user.to_string())]
    }

    async fn send(
        app: &axum::Router,
        user: UserId,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> Response {
        let mut builder = Request::builder().method(method).uri(uri);
        for (name, value) in caller_headers(user) {
            builder = builder.header(name, value);
        }
        let request = match body {
            Some(value) => builder
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&value).unwrap()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        app.clone().oneshot(request).await.unwrap()
    }

    async fn ensure(app: &axum::Router, user: UserId, project: ProjectId) -> Response {
        send(
            app,
            user,
            "POST",
            "/api/v1/workspaces",
            Some(json!({ "project_id": project.to_string() })),
        )
        .await
    }

    async fn json_of(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn a_request_without_an_identity_is_unauthorized() {
        let project = ProjectId::new();
        let app = open_app(project);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workspaces")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "project_id": project.to_string() })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn an_unparsable_identity_is_rejected() {
        let project = ProjectId::new();
        let app = open_app(project);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workspaces")
                    .header("x-menzi-user-id", "not-a-uuid")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "project_id": project.to_string() })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn ensure_returns_a_ready_workspace() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        let response = ensure(&app, user, project).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert_eq!(body["status"], "ready");
        assert!(body["instance_name"].as_str().unwrap().starts_with("wsp-"));
        assert!(body["endpoint"].as_str().is_some());
    }

    #[tokio::test]
    async fn the_identity_header_wins_over_a_body_user() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        let response = send(
            &app,
            user,
            "POST",
            "/api/v1/workspaces",
            Some(json!({
                "project_id": project.to_string(),
                "user_id": UserId::new().to_string(),
            })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_user_outside_the_project_is_forbidden() {
        let user = UserId::new();
        let app = open_app(ProjectId::new());
        let response = ensure(&app, user, ProjectId::new()).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_service_principal_must_name_the_user() {
        let project = ProjectId::new();
        let h = harness();
        let api = WorkspaceApiState::new(Arc::new(h.manager.clone()), "mz-workspace");
        let app = create_router(api);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workspaces")
                    .header("x-menzi-service", "autonomous")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "project_id": project.to_string() })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_service_principal_may_name_a_user() {
        let project = ProjectId::new();
        let user = UserId::new();
        let h = harness();
        let api = WorkspaceApiState::new(Arc::new(h.manager.clone()), "mz-workspace");
        let app = create_router(api);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workspaces")
                    .header("x-menzi-service", "autonomous")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "project_id": project.to_string(),
                            "user_id": user.to_string(),
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn the_path_user_must_match_the_caller() {
        let project = ProjectId::new();
        let owner = UserId::new();
        let app = open_app(project);
        ensure(&app, owner, project).await;
        let response = send(
            &app,
            UserId::new(),
            "GET",
            &format!("/api/v1/workspaces/{owner}/{project}"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn get_returns_not_found_before_anything_is_provisioned() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn connect_returns_the_endpoint() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/connect"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert!(body["endpoint"].as_str().is_some());
    }

    #[tokio::test]
    async fn prompt_without_a_session_opens_one() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/prompt"),
            Some(json!({ "text": "hello" })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await["session_id"], "ses_1");
    }

    #[tokio::test]
    async fn a_session_opens_without_a_prompt() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/sessions"),
            Some(json!({ "title": "smoke" })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(json_of(response).await["id"], "ses_1");
    }

    #[tokio::test]
    async fn sessions_are_listed_from_the_workspace_opencode() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}/sessions"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await.as_array().unwrap().len(), 1);
        let sessions = json_of(
            send(
                &app,
                user,
                "GET",
                &format!("/api/v1/workspaces/{user}/{project}/sessions"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            sessions[0].get("directory").and_then(|d| d.as_str()),
            Some("/workspace")
        );
    }

    #[tokio::test]
    async fn the_diff_route_reports_the_working_tree() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}/diff"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert!(body.get("head").is_some(), "the commit it compares against");
        assert!(body.get("version").is_some(), "the exact git state snapshot");
        assert!(body.get("changes").unwrap().is_array());
    }

    #[tokio::test]
    async fn the_diff_route_rejects_a_stale_version_for_a_patch_request() {
        let project = ProjectId::new();
        let user = UserId::new();
        let (app, h) = open_app_with_harness(project);
        ensure(&app, user, project).await;
        h.driver.answers(status_cmd(), " M src/a.rs\0");
        h.driver.answers(numstat_all_cmd(), "1\t1\tsrc/a.rs\n");
        h.driver.answers(head_cmd(), "abc123\n");

        let list = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}/diff"),
            None,
        )
        .await;
        let version = json_of(list).await["version"].as_str().unwrap().to_string();

        h.driver.answers(status_cmd(), " M src/a.rs\0 M src/b.rs\0");
        h.driver
            .answers(numstat_all_cmd(), "1\t1\tsrc/a.rs\n2\t0\tsrc/b.rs\n");
        let response = send(
            &app,
            user,
            "GET",
            &format!(
                "/api/v1/workspaces/{user}/{project}/diff?path=src%2Fa.rs&version={version}"
            ),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn the_diff_route_needs_an_identity() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workspaces/{user}/{project}/diff"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn terminal_returns_the_command_output() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/terminal"),
            Some(json!({ "command": "ls", "args": ["-la"] })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert_eq!(body["exit_code"], 0);
        assert_eq!(body["stdout"], "ok");
    }

    #[tokio::test]
    async fn destroy_marks_the_workspace_deleted() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "DELETE",
            &format!("/api/v1/workspaces/{user}/{project}"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}"),
            None,
        )
        .await;
        assert_eq!(json_of(response).await["status"], "deleted");
    }

    #[tokio::test]
    async fn suspend_and_start_cycle_the_workspace() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await["status"], "idle");
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/start"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn listing_a_projects_workspaces_is_gated_by_membership() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/projects/{project}/workspaces"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await.as_array().unwrap().len(), 1);

        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/projects/{}/workspaces", ProjectId::new()),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_sessions_workspace_lookup_names_the_endpoint() {
        let project = ProjectId::new();
        let user = UserId::new();
        let h = harness();
        let api = WorkspaceApiState::new(Arc::new(h.manager.clone()), "mz-workspace")
            .with_authorizer(Arc::new(
                crate::identity::MembershipAuthorizer::new().with_project(project),
            ));
        let app = create_router(api);
        ensure(&app, user, project).await;
        let created = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/sessions"),
            Some(json!({ "title": "smoke" })),
        )
        .await;
        let session = json_of(created).await;
        let session_id = session["id"].as_str().unwrap().to_string();

        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}/sessions/{session_id}/workspace"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert_eq!(body["endpoint"], "http://10.0.0.9:17999");
        assert_eq!(body["opencode_session"], session_id);
        assert_eq!(body["kind"], "interactive");
    }

    #[tokio::test]
    async fn a_sessions_workspace_lookup_for_an_unknown_session_is_not_found() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        let response = send(
            &app,
            user,
            "GET",
            &format!("/api/v1/workspaces/{user}/{project}/sessions/ses_nope/workspace"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_service_principal_can_route_a_session_it_holds_no_user_for() {
        let project = ProjectId::new();
        let user = UserId::new();
        let h = harness();
        let api = WorkspaceApiState::new(Arc::new(h.manager.clone()), "mz-workspace")
            .with_authorizer(Arc::new(
                crate::identity::MembershipAuthorizer::new().with_project(project),
            ));
        let app = create_router(api);
        ensure(&app, user, project).await;
        let created = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/sessions"),
            Some(json!({ "title": "e2e" })),
        )
        .await;
        let session_id = json_of(created).await["id"].as_str().unwrap().to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/sessions/{session_id}/workspace"))
                    .header("x-menzi-service", "autonomous")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert_eq!(body["endpoint"], "http://10.0.0.9:17999");
    }

    #[tokio::test]
    async fn a_user_cannot_route_someone_elses_session() {
        let project = ProjectId::new();
        let user = UserId::new();
        let h = harness();
        let api = WorkspaceApiState::new(Arc::new(h.manager.clone()), "mz-workspace")
            .with_authorizer(Arc::new(
                crate::identity::MembershipAuthorizer::new().with_project(project),
            ));
        let app = create_router(api);
        ensure(&app, user, project).await;
        let created = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/sessions"),
            Some(json!({ "title": "e2e" })),
        )
        .await;
        let session_id = json_of(created).await["id"].as_str().unwrap().to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/sessions/{session_id}/workspace"))
                    .header("x-menzi-user-id", UserId::new().to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_sessions_workspace_lookup_is_gated_by_membership() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        let response = send(
            &app,
            user,
            "GET",
            &format!(
                "/api/v1/workspaces/{user}/{}/sessions/ses_1/workspace",
                ProjectId::new()
            ),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn interrupt_answers_for_the_open_session() {
        let project = ProjectId::new();
        let user = UserId::new();
        let app = open_app(project);
        ensure(&app, user, project).await;
        let response = send(
            &app,
            user,
            "POST",
            &format!("/api/v1/workspaces/{user}/{project}/interrupt"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
