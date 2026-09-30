use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use utoipa::OpenApi;

pub mod auth;
pub mod identity;
pub mod modules;

#[derive(OpenApi)]
#[openapi(
    paths(
        modules::health::health_check,
        modules::projects::list_projects,
        modules::projects::create_project,
        modules::projects::get_project,
    ),
    components(
        schemas(
            modules::health::HealthResponse,
            modules::projects::ProjectResponse,
            modules::projects::CreateProjectRequest,
        ),
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "projects", description = "Project management endpoints"),
    )
)]
pub struct ApiDoc;

fn application_routes() -> Router<PgPool> {
    Router::new()
        .route(
            "/api/v1/projects",
            get(modules::projects::list_projects).post(modules::projects::create_project),
        )
        .route("/api/v1/projects/{id}", get(modules::projects::get_project))
        .route(
            "/api/v1/projects/{project_id}/previews",
            get(modules::previews_proxy::forward_previews),
        )
        .route(
            "/api/v1/previews",
            post(modules::previews_proxy::forward_previews),
        )
        .route(
            "/api/v1/previews/{id}",
            get(modules::previews_proxy::forward_previews)
                .delete(modules::previews_proxy::forward_previews),
        )
        .route(
            "/api/v1/previews/{id}/reset",
            post(modules::previews_proxy::forward_previews),
        )
        .route(
            "/api/v1/previews/{id}/restart",
            post(modules::previews_proxy::forward_previews),
        )
        .route(
            "/api/v1/workspaces",
            post(modules::workspaces_proxy::forward_workspaces),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}",
            get(modules::workspaces_proxy::forward_workspaces)
                .post(modules::workspaces_proxy::forward_workspaces)
                .delete(modules::workspaces_proxy::forward_workspaces),
        )
        .route(
            "/api/v1/workspaces/{user_id}/{project_id}/{*rest}",
            get(modules::workspaces_proxy::forward_workspaces)
                .post(modules::workspaces_proxy::forward_workspaces)
                .delete(modules::workspaces_proxy::forward_workspaces),
        )
        .route(
            "/api/v1/projects/{project_id}/workspaces",
            get(modules::workspaces_proxy::forward_workspaces),
        )
}

pub fn create_router(auth: Arc<auth::AuthState>) -> Router<PgPool> {
    let config = auth.config.clone();
    let resolver = auth.resolver.clone();
    let application = application_routes()
        .layer(axum::middleware::from_fn_with_state(
            config,
            auth::middleware::require_csrf,
        ))
        .layer(axum::middleware::from_fn_with_state(
            resolver,
            auth::middleware::require_caller,
        ));
    Router::new()
        .route("/health", get(modules::health::health_check))
        .merge(auth::handlers::merge(auth))
        .merge(application)
        .layer(axum::middleware::from_fn(
            auth::middleware::strip_identity_headers,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn router() -> Router {
        create_router(Arc::new(auth::AuthState::for_tests())).with_state(lazy_pool())
    }

    fn lazy_pool() -> PgPool {
        PgPool::connect_lazy("postgres://unused:unused@127.0.0.1:1/unused").expect("lazy pool")
    }

    async fn call(app: Router, uri: &str) -> axum::http::StatusCode {
        app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[test]
    fn api_doc_generates() {
        let doc = ApiDoc::openapi();
        assert!(doc.paths.paths.contains_key("/health"));
        assert!(doc.paths.paths.contains_key("/api/v1/projects"));
    }

    #[tokio::test]
    async fn the_health_check_is_public() {
        assert_eq!(call(router(), "/health").await, axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn the_login_page_can_read_the_providers() {
        assert_eq!(
            call(router(), "/api/v1/auth/providers").await,
            axum::http::StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_protected_route_without_a_session_is_refused() {
        assert_eq!(
            call(router(), "/api/v1/projects").await,
            axum::http::StatusCode::UNAUTHORIZED
        );
    }
}
