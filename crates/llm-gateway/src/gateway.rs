use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use menzi_auth::tenant::{TenantContext, HEADER_PROJECT_ID, HEADER_SESSION_ID};
use menzi_common::ids::{ProjectId, SessionId, UserId};
use serde_json::json;
use std::sync::Arc;

use super::adapters::*;
use super::audit::*;
use super::budgets::*;
use super::cost::*;
use super::policy::*;
use super::state_store::*;
use super::types::*;

#[derive(Clone)]
pub struct GatewayState {
    pub factory: Arc<ProviderAdapterFactory>,
    pub store: Arc<dyn GatewayStateStore>,
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
        let mut store = InMemoryGatewayStateStore::new();
        for (scope, config) in policy.export() {
            store = store.with_policy(scope, config);
        }
        for (key, budget) in budgets.export() {
            store = store.with_budget(key, budget);
        }
        Self {
            factory: Arc::new(factory),
            store: Arc::new(store),
            cost: Arc::new(cost),
            audit: Arc::new(audit),
            default_provider: default_provider.into(),
        }
    }

    pub fn with_store(mut self, store: Arc<dyn GatewayStateStore>) -> Self {
        self.store = store;
        self
    }

    pub async fn budget(&self, key: &str) -> Option<BudgetStatus> {
        self.store.budget(key).await.ok().flatten()
    }
}

pub fn create_router(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/models", get(list_models))
        .with_state(state)
}

async fn health_check() -> Response {
    (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
}

async fn ready_check(State(state): State<GatewayState>) -> Response {
    match state.store.budget("global").await {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "ready"}))).into_response(),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "degraded", "error": error.to_string()})),
        )
            .into_response(),
    }
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

fn tenant_ids(headers: &HeaderMap) -> (Option<ProjectId>, Option<UserId>) {
    match TenantContext::from_headers(headers) {
        Ok(tenant) => (tenant.project_id, tenant.user_id),
        Err(_) => (None, None),
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

    let allowed_policy = match state.store.is_model_allowed(&scope, &request.model).await {
        Ok(allowed) => allowed,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response();
        }
    };

    if !allowed_policy {
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
    let budget_scope = budget_key(&headers);
    let allowed_budget = match state.store.can_spend(&budget_scope, estimated_cost).await {
        Ok(allowed) => allowed,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response();
        }
    };
    if !allowed_budget {
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
            if let Err(error) = state.store.record_spend(&budget_scope, actual_cost).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": error.to_string()})),
                )
                    .into_response();
            }

            let (project_id, user_id) = tenant_ids(&headers);
            let record = state.audit.log_request(LogRequestParams {
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
            if let Err(error) = state.store.record_usage(&budget_scope, &record).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": error.to_string()})),
                )
                    .into_response();
            }
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
    use tower::ServiceExt;

    fn budget_manager() -> BudgetManager {
        let mut manager = BudgetManager::new();
        manager.set_budget(
            "global".to_string(),
            BudgetStatus {
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
    async fn health_and_ready_endpoints_are_available() {
        let (app, _state) = gateway().await;
        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let ready = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ready.status(), StatusCode::OK);
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

        let spent = state.budget("global").await.unwrap().spent_usd;
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
        let mut store = InMemoryGatewayStateStore::new();
        store = store.with_budget(
            "global".to_string(),
            BudgetStatus {
                project_id: None,
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 1.0,
                budget_usd: 0.0,
            },
        );
        let state = state.with_store(Arc::new(store));
        let app = create_router(state);
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

    fn tenant_request(path: &str, project_id: ProjectId) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
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
        let project_a = ProjectId::new();
        let scope_a = format!("project:{project_a}");
        let mut store = InMemoryGatewayStateStore::new();
        store = store.with_budget(
            scope_a,
            BudgetStatus {
                project_id: Some(project_a),
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 100.0,
                budget_usd: 100.0,
            },
        );
        let state = state.with_store(Arc::new(store));
        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(tenant_request("/v1/chat/completions", project_a))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let project_b = ProjectId::new();
        let response = app
            .oneshot(tenant_request("/v1/chat/completions", project_b))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn shared_store_is_visible_to_all_replicas() {
        let (_app, state_a) = gateway().await;
        let (_app, state_b) = gateway().await;
        let project_id = ProjectId::new();
        let scope = format!("project:{project_id}");
        let mut store = InMemoryGatewayStateStore::new();
        store = store.with_budget(
            scope,
            BudgetStatus {
                project_id: Some(project_id),
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 1.0,
                budget_usd: 0.0,
            },
        );
        let shared: Arc<dyn GatewayStateStore> = Arc::new(store);
        let app_a = create_router(state_a.with_store(shared.clone()));
        let app_b = create_router(state_b.with_store(shared));

        let response_a = app_a
            .clone()
            .oneshot(tenant_request("/v1/chat/completions", project_id))
            .await
            .unwrap();
        assert_eq!(response_a.status(), StatusCode::TOO_MANY_REQUESTS);

        let response_b = app_b
            .oneshot(tenant_request("/v1/chat/completions", project_id))
            .await
            .unwrap();
        assert_eq!(response_b.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn chat_completions_applies_tenant_policy() {
        let project_id = ProjectId::new();
        let scope = format!("project:{project_id}");
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
            .oneshot(tenant_request("/v1/chat/completions", project_id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn chat_completions_records_tenant_in_audit() {
        let (app, state) = gateway().await;
        let project_id = ProjectId::new();
        let response = app
            .oneshot(tenant_request("/v1/chat/completions", project_id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let record = state.audit.last_record().expect("audit record");
        assert_eq!(record.project_id, Some(project_id));
    }

    #[test]
    fn request_scope_falls_back_without_tenant_headers() {
        let headers = HeaderMap::new();
        assert_eq!(request_scope(&headers), "default");
        assert_eq!(budget_key(&headers), "global");
    }

    #[test]
    fn request_scope_uses_an_unparsable_project_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-menzi-project-id", "proj-1".parse().unwrap());
        assert_eq!(request_scope(&headers), "proj-1");
    }

    #[test]
    fn request_scope_uses_tenant_scope_key() {
        let project_id = ProjectId::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-menzi-project-id",
            project_id.to_string().parse().unwrap(),
        );
        assert_eq!(request_scope(&headers), format!("project:{project_id}"));
        assert_eq!(budget_key(&headers), format!("project:{project_id}"));
    }
}
