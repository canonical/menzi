use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::identity::{caller_uuid, Caller};

#[derive(Serialize, ToSchema)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ListProjectsQuery {
    pub search: Option<String>,
}

const MAX_NAME_LENGTH: usize = 120;

type ProjectRow = (String, String, String, Option<String>, Option<String>, String, String);

fn map_row(row: ProjectRow) -> ProjectResponse {
    ProjectResponse {
        id: row.0,
        name: row.1,
        slug: row.2,
        description: row.3,
        repository_url: row.4,
        created_at: row.5,
        updated_at: row.6,
    }
}

fn looks_like_ssh_repository(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }
    let scheme = trimmed.starts_with("ssh://");
    let scp = trimmed.contains('@') && trimmed.contains(':') && !trimmed.contains(" ");
    let git_at = trimmed.starts_with("git@") && trimmed.contains(':');
    (scheme || scp || git_at) && trimmed.ends_with(".git")
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "projects",
    params(
        ("search" = Option<String>, Query, description = "Filter by name or slug")
    ),
    responses(
        (status = 200, description = "Projects the caller is a member of", body = Vec<ProjectResponse>),
        (status = 401, description = "No caller established")
    )
)]
pub async fn list_projects(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Query(query): Query<ListProjectsQuery>,
) -> Response {
    let Some(caller_id) = caller_uuid(&caller) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "who is calling is not established",
        );
    };

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", value.to_lowercase()));

    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT p.id::text, p.name, p.slug, p.description, p.repository_url, p.created_at::text, \
         p.updated_at::text \
         FROM projects p \
         JOIN project_members m ON m.project_id = p.id \
         WHERE m.user_id = $1::uuid \
         AND ($2::text IS NULL OR lower(p.name) LIKE $2::text OR lower(p.slug) LIKE $2::text) \
         GROUP BY p.id \
         ORDER BY p.updated_at DESC",
    )
    .bind(caller_id)
    .bind(search.as_deref())
    .fetch_all(&pool)
    .await;

    match rows {
        Ok(rows) => (
            StatusCode::OK,
            Json(rows.into_iter().map(map_row).collect::<Vec<_>>()),
        )
            .into_response(),
        Err(error) => {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/projects",
    tag = "projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, description = "Project created", body = ProjectResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Slug already used")
    )
)]
pub async fn create_project(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Json(body): Json<CreateProjectRequest>,
) -> Response {
    let Some(caller_id) = caller_uuid(&caller) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "who is calling is not established",
        );
    };
    if body.name.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "name must not be empty");
    }
    if body.name.trim().len() > MAX_NAME_LENGTH {
        return error_response(
            StatusCode::BAD_REQUEST,
            "name must be at most 120 characters",
        );
    }
    if body.slug.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "slug must not be empty");
    }
    let repository_url = body
        .repository_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(url) = repository_url {
        if !looks_like_ssh_repository(url) {
            return error_response(
                StatusCode::BAD_REQUEST,
                "repository_url must be an SSH git URL ending in .git",
            );
        }
    }
    if !body
        .slug
        .trim()
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return error_response(
            StatusCode::BAD_REQUEST,
            "slug may only contain lowercase letters, digits and hyphens",
        );
    }

    let row = sqlx::query_as::<_, ProjectRow>(
        "INSERT INTO projects (name, slug, description, repository_url) VALUES ($1, $2, $3, $4) \
         RETURNING id::text, name, slug, description, repository_url, created_at::text, updated_at::text",
    )
    .bind(body.name.trim())
    .bind(body.slug.trim().to_lowercase())
    .bind(body.description.as_deref())
    .bind(repository_url)
    .fetch_one(&pool)
    .await;

    let created = match row {
        Ok(row) => map_row(row),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            return error_response(
                StatusCode::CONFLICT,
                "a project with this slug already exists",
            );
        }
        Err(error) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string());
        }
    };

    if let Err(error) = sqlx::query(
        "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(Uuid::parse_str(&created.id).ok())
    .bind(caller_id)
    .execute(&pool)
    .await
    {
        tracing::warn!("could not record the project owner: {error}");
    }

    (StatusCode::CREATED, Json(created)).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    params(("id" = String, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Project details", body = ProjectResponse),
        (status = 404, description = "Project not found")
    )
)]
pub async fn get_project(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<String>,
) -> Response {
    let Some(caller_id) = caller_uuid(&caller) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "who is calling is not established",
        );
    };
    if Uuid::parse_str(&id).is_err() {
        return error_response(StatusCode::NOT_FOUND, "project not found");
    }

    let row = sqlx::query_as::<_, ProjectRow>(
        "SELECT p.id::text, p.name, p.slug, p.description, p.repository_url, p.created_at::text, p.updated_at::text \
         FROM projects p \
         JOIN project_members m ON m.project_id = p.id \
         WHERE p.id = $1::uuid AND m.user_id = $2::uuid",
    )
    .bind(Uuid::parse_str(&id).ok())
    .bind(caller_id)
    .fetch_optional(&pool)
    .await;

    match row {
        Ok(Some(row)) => (StatusCode::OK, Json(map_row(row))).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "project not found").into_response(),
        Err(error) => {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::looks_like_ssh_repository;

    #[test]
    fn accepts_ssh_git_urls() {
        assert!(looks_like_ssh_repository("git@github.com:acme/repo.git"));
        assert!(looks_like_ssh_repository("ssh://git@gitlab.com/acme/repo.git"));
    }

    #[test]
    fn rejects_non_ssh_or_non_git_urls() {
        assert!(!looks_like_ssh_repository("https://github.com/acme/repo.git"));
        assert!(!looks_like_ssh_repository("git@github.com:acme/repo"));
    }
}
