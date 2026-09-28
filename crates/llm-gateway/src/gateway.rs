use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use menzi_auth::tenant::{TenantContext, HEADER_PROJECT_ID, HEADER_SESSION_ID};
use menzi_common::ids::{OrgId, ProjectId, SessionId, UserId};
use serde_json::json;
use std::sync::{Arc, Mutex};

use super::adapters::*;
use super::audit::*;
use super::budgets::*;
use super::cost::*;
use super::policy::*;
use super::types::*;

#[derive(Clone)]
pub struct GatewayState {
    pub factory: Arc<ProviderAdapterFactory>,
    pub policy: Arc<PolicyEngine>,
    pub budgets: Arc<Mutex<BudgetManager>>,
    pub cost: Arc<CostCalculator>,
    pub audit: Arc<AuditLogger>,
    pub default_provider: String,
}

impl GatewayState {
    pub fn new(
        factory: ProviderAdapterFactory,
        policy: PolicyEngine,
        budgets: BudgetManager,
        cost: CostCalculator,
        audit: AuditLogger,
        default_provider: impl Into<String>,
    ) -> Self {
        Self {
            factory: Arc::new(factory),
            policy: Arc::new(policy),
            budgets: Arc::new(Mutex::new(budgets)),
            cost: Arc::new(cost),
            audit: Arc::new(audit),
            default_provider: default_provider.into(),
        }
    }
}

pub fn create_router(state: GatewayState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/models", get(list_models))
        .with_state(state)
}

fn provider_for_model(model: &str, default: &str) -> String {
    if model.contains('/') {
        model.split('/').next().unwrap_or(default).to_string()
    } else {
        default.to_string()
    }
}

fn header_value(headers: &HeaderMap, name: &'static str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn request_scope(headers: &HeaderMap) -> String {
    match TenantContext::from_headers(headers) {
        Ok(tenant) => tenant.scope_key(),
        Err(_) => {
            let project = header_value(headers, HEADER_PROJECT_ID);
            if project.is_empty() {
                TenantContext::default_scope()
            } else {
                project
            }
        }
    }
}

fn budget_key(headers: &HeaderMap) -> String {
    match TenantContext::from_headers(headers) {
        Ok(tenant) => tenant.scope_key(),
        Err(_) => "global".to_string(),
    }
}

fn tenant_ids(headers: &HeaderMap) -> (Option<OrgId>, Option<ProjectId>, Option<UserId>) {
    match TenantContext::from_headers(headers) {
        Ok(tenant) => (Some(tenant.org_id), tenant.project_id, tenant.user_id),
        Err(_) => (None, None, None),
    }
}

fn session_id_from_header(headers: &HeaderMap) -> Option<SessionId> {
    header_value(headers, HEADER_SESSION_ID).parse().ok()
}

async fn chat_completions(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(request): Json<ChatRequest>,
) -> Response {
    let scope = request_scope(&headers);

    if !state.policy.is_model_allowed(&scope, &request.model) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "model not allowed by policy"})),
        )
            .into_response();
    }

    let provider = provider_for_model(&request.model, &state.default_provider);
    let adapter = state
        .factory
        .select(&provider)
        .or_else(|| state.factory.select(&state.default_provider));
    let Some(adapter) = adapter else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("no adapter registered for provider '{provider}'")})),
        )
            .into_response();
    };

    let estimate_source = request
        .messages
        .last()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let estimated_tokens = state.cost.estimate_tokens(&estimate_source);
    let estimated_cost = state.cost.calculate(estimated_tokens, 0, &request.model);
    let allowed = {
        let budgets = state.budgets.lock().expect("budgets lock");
        budgets.can_spend(&budget_key(&headers), estimated_cost)
    };
    if !allowed {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "budget exceeded"})),
        )
            .into_response();
    }

    match adapter.chat(&request).await {
        Ok(response) => {
            let actual_cost = state.cost.calculate(
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                &request.model,
            );
            let mut budgets = state.budgets.lock().expect("budgets lock");
            budgets.record_spend(&budget_key(&headers), actual_cost);
            drop(budgets);

            let (org_id, project_id, user_id) = tenant_ids(&headers);
            let record = state.audit.log_request(LogRequestParams {
                org_id,
                project_id,
                user_id,
                session_id: session_id_from_header(&headers),
                feature: "chat".to_string(),
                provider,
                model: request.model.clone(),
                input_tokens: response.usage.prompt_tokens,
                output_tokens: response.usage.completion_tokens,
                cost: actual_cost,
            });
            tracing::debug!("usage recorded: {:?}", record);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn embeddings(
    State(state): State<GatewayState>,
    Json(request): Json<EmbedRequest>,
) -> Response {
    let provider = provider_for_model(&request.model, &state.default_provider);
    let adapter = state
        .factory
        .select(&provider)
        .or_else(|| state.factory.select(&state.default_provider));
    let Some(adapter) = adapter else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("no adapter registered for provider '{provider}'")})),
        )
            .into_response();
    };
    match adapter.embed(&request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn list_models(State(state): State<GatewayState>) -> Response {
    let providers: Vec<ProviderInfo> = state.factory.list().into_iter().cloned().collect();
    (StatusCode::OK, Json(providers)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use menzi_common::ids::OrgId;
    use tower::ServiceExt;

    fn budget_manager() -> BudgetManager {
        let mut manager = BudgetManager::new();
        manager.set_budget(
            "global".to_string(),
            BudgetStatus {
                org_id: OrgId::new(),
                project_id: None,
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 0.0,
                budget_usd: 100.0,
            },
        );
        manager
    }

    async fn start_fake_provider() -> String {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "id": "chatcmpl-fake",
                    "model": "openrouter/mock-gpt",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "hello"},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    async fn gateway() -> (Router, GatewayState) {
        gateway_with_policy(PolicyEngine::new()).await
    }

    async fn gateway_with_policy(policy: PolicyEngine) -> (Router, GatewayState) {
        let target = start_fake_provider().await;
        let mut factory = ProviderAdapterFactory::new();
        factory.register_adapter(
            "openrouter",
            Arc::new(OpenAICompatibleAdapter::new(
                format!("{target}/v1"),
                "key",
                "openrouter",
            )),
        );
        factory.register(ProviderInfo {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            models: vec!["mock-gpt".to_string()],
        });
        let state = GatewayState::new(
            factory,
            policy,
            budget_manager(),
            CostCalculator,
            AuditLogger::new(),
            "openrouter",
        );
        let app = create_router(state.clone());
        (app, state)
    }

    async fn body_string(response: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[tokio::test]
    async fn chat_completions_forwards_and_records_spend() {
        let (app, state) = gateway().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "model": "openrouter/mock-gpt",
                            "messages": [{"role": "user", "content": "hi"}],
                            "stream": false
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("chatcmpl-fake"));

        let spent = {
            let budgets = state.budgets.lock().unwrap();
            budgets.check_budget("global").unwrap().spent_usd
        };
        assert!(spent > 0.0);
    }

    #[tokio::test]
    async fn chat_completions_blocks_denied_model() {
        let mut policy = PolicyEngine::new();
        policy.register(
            "default".to_string(),
            PolicyConfig {
                allowed_models: vec!["openrouter/*".to_string()],
                denied_models: vec!["*/mock-gpt".to_string()],
                data_classification: DataClassification::Internal,
            },
        );
        let (app, _state) = gateway_with_policy(policy).await;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "model": "openrouter/mock-gpt",
                            "messages": [{"role": "user", "content": "hi"}],
                            "stream": false
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn chat_completions_blocks_when_budget_exhausted() {
        let (app, state) = gateway().await;
        {
            let mut budgets = state.budgets.lock().unwrap();
            budgets.set_budget(
                "global".to_string(),
                BudgetStatus {
                    org_id: OrgId::new(),
                    project_id: None,
                    user_id: None,
                    session_id: None,
                    feature: "coding".to_string(),
                    spent_usd: 100.0,
                    budget_usd: 100.0,
                },
            );
        }
        let long_prompt = "a".repeat(400);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "model": "openrouter/mock-gpt",
                            "messages": [{"role": "user", "content": long_prompt}],
                            "stream": false
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn list_models_returns_registered_providers() {
        let (app, _state) = gateway().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("openrouter"));
    }

    #[test]
    fn provider_for_model_parses_prefix() {
        assert_eq!(
            provider_for_model("openrouter/anthropic/claude", "local"),
            "openrouter"
        );
        assert_eq!(provider_for_model("gpt-4", "local"), "local");
    }

    fn tenant_request(path: &str, org_id: OrgId, project_id: ProjectId) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .header("x-menzi-org-id", org_id.to_string())
            .header("x-menzi-project-id", project_id.to_string())
            .header("x-menzi-user-id", UserId::new().to_string())
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "model": "openrouter/mock-gpt",
                    "messages": [{"role": "user", "content": "hello"}],
                    "stream": false
                }))
                .unwrap(),
            ))
            .unwrap()
    }

    #[tokio::test]
    async fn chat_completions_scopes_budget_per_tenant() {
        let (app, state) = gateway().await;
        let org_a = OrgId::new();
        let project_a = ProjectId::new();
        let scope_a = format!("{org_a}:{project_a}");
        {
            let mut budgets = state.budgets.lock().unwrap();
            budgets.set_budget(
                scope_a,
                BudgetStatus {
                    org_id: org_a,
                    project_id: Some(project_a),
                    user_id: None,
                    session_id: None,
                    feature: "coding".to_string(),
                    spent_usd: 100.0,
                    budget_usd: 100.0,
                },
            );
        }

        let response = app
            .clone()
            .oneshot(tenant_request("/v1/chat/completions", org_a, project_a))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let org_b = OrgId::new();
        let project_b = ProjectId::new();
        let response = app
            .oneshot(tenant_request("/v1/chat/completions", org_b, project_b))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn chat_completions_applies_tenant_policy() {
        let org_id = OrgId::new();
        let project_id = ProjectId::new();
        let scope = format!("{org_id}:{project_id}");
        let mut policy = PolicyEngine::new();
        policy.register(
            scope,
            PolicyConfig {
                allowed_models: vec!["*".to_string()],
                denied_models: vec!["*/mock-gpt".to_string()],
                data_classification: DataClassification::Internal,
            },
        );
        let (app, _state) = gateway_with_policy(policy).await;

        let response = app
            .oneshot(tenant_request("/v1/chat/completions", org_id, project_id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn chat_completions_records_tenant_in_audit() {
        let (app, state) = gateway().await;
        let org_id = OrgId::new();
        let project_id = ProjectId::new();
        let response = app
            .oneshot(tenant_request("/v1/chat/completions", org_id, project_id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let record = state.audit.last_record().expect("audit record");
        assert_eq!(record.org_id, org_id);
        assert_eq!(record.project_id, Some(project_id));
    }

    #[test]
    fn request_scope_falls_back_without_tenant_headers() {
        let headers = HeaderMap::new();
        assert_eq!(request_scope(&headers), "default");
        assert_eq!(budget_key(&headers), "global");
    }

    #[test]
    fn request_scope_uses_project_header_without_org() {
        let mut headers = HeaderMap::new();
        headers.insert("x-menzi-project-id", "proj-1".parse().unwrap());
        assert_eq!(request_scope(&headers), "proj-1");
    }

    #[test]
    fn request_scope_uses_tenant_scope_key() {
        let org_id = OrgId::new();
        let project_id = ProjectId::new();
        let mut headers = HeaderMap::new();
        headers.insert("x-menzi-org-id", org_id.to_string().parse().unwrap());
        headers.insert(
            "x-menzi-project-id",
            project_id.to_string().parse().unwrap(),
        );
        assert_eq!(request_scope(&headers), format!("{org_id}:{project_id}"));
        assert_eq!(budget_key(&headers), format!("{org_id}:{project_id}"));
    }
}
