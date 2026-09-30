use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use utoipa::OpenApi;

pub mod identity;
pub mod modules;
pub mod whoami;

#[derive(OpenApi)]
#[openapi(
    paths(
        modules::health::health_check,
        modules::orgs::list_orgs,
        modules::orgs::create_org,
        modules::orgs::get_org,
        modules::projects::list_projects,
        modules::projects::create_project,
        modules::projects::get_project,
    ),
    components(
        schemas(
            modules::health::HealthResponse,
            modules::orgs::OrgResponse,
            modules::orgs::CreateOrgRequest,
            modules::projects::ProjectResponse,
            modules::projects::CreateProjectRequest,
        ),
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "orgs", description = "Org management endpoints"),
        (name = "projects", description = "Project management endpoints"),
    )
)]
pub struct ApiDoc;

pub fn create_router() -> Router<PgPool> {
    Router::new()
        .route("/health", get(modules::health::health_check))
        .route("/api/v1/me", get(whoami::whoami))
        .route(
            "/api/v1/orgs",
            get(modules::orgs::list_orgs).post(modules::orgs::create_org),
        )
        .route("/api/v1/orgs/{id}", get(modules::orgs::get_org))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_doc_generates() {
        let doc = ApiDoc::openapi();
        assert!(doc.paths.paths.contains_key("/health"));
        assert!(doc.paths.paths.contains_key("/api/v1/orgs"));
    }
}
