use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct ProjectResponse {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    pub org_id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ListProjectsQuery {
    pub org_id: Option<String>,
    pub search: Option<String>,
}

const MAX_NAME_LENGTH: usize = 120;

fn map_row(
    row: (
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
    ),
) -> ProjectResponse {
    ProjectResponse {
        id: row.0,
        org_id: row.1,
        name: row.2,
        slug: row.3,
        description: row.4,
        created_at: row.5,
        updated_at: row.6,
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "projects",
    params(
        ("org_id" = Option<String>, Query, description = "Filter by org"),
        ("search" = Option<String>, Query, description = "Filter by name or slug")
    ),
    responses(
        (status = 200, description = "List projects", body = Vec<ProjectResponse>)
    )
)]
pub async fn list_projects(
    State(pool): State<PgPool>,
    Query(query): Query<ListProjectsQuery>,
) -> Response {
    let search = query
        .search
        .map(|value| format!("%{}%", value.trim().to_lowercase()));
    let org_filter = match query.org_id.as_deref() {
        Some(value) if Uuid::parse_str(value).is_ok() => Some(Uuid::parse_str(value).ok()),
        Some(_) => {
            return error_response(StatusCode::BAD_REQUEST, "org_id must be a uuid").into_response()
        }
        None => None,
    };

    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id::text, org_id::text, name, slug, description, created_at::text, \
         updated_at::text FROM projects \
         WHERE ($1::uuid IS NULL OR org_id = $1::uuid) \
         AND ($2::text IS NULL OR lower(name) LIKE $2::text OR lower(slug) LIKE $2::text) \
         ORDER BY updated_at DESC",
    )
    .bind(org_filter)
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
        (status = 404, description = "Org not found"),
        (status = 409, description = "Slug already used in this org")
    )
)]
pub async fn create_project(
    State(pool): State<PgPool>,
    Json(body): Json<CreateProjectRequest>,
) -> Response {
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
    if Uuid::parse_str(&body.org_id).is_err() {
        return error_response(StatusCode::BAD_REQUEST, "org_id must be a uuid");
    }

    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
        ),
    >(
        "INSERT INTO projects (org_id, name, slug, description) VALUES ($1, $2, $3, $4) \
         RETURNING id::text, org_id::text, name, slug, description, created_at::text, \
         updated_at::text",
    )
    .bind(Uuid::parse_str(&body.org_id).ok())
    .bind(body.name.trim())
    .bind(body.slug.trim().to_lowercase())
    .bind(body.description.as_deref())
    .fetch_one(&pool)
    .await;

    match row {
        Ok(row) => (StatusCode::CREATED, Json(map_row(row))).into_response(),
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            error_response(StatusCode::NOT_FOUND, "org not found").into_response()
        }
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            error_response(
                StatusCode::CONFLICT,
                "a project with this slug already exists in this org",
            )
            .into_response()
        }
        Err(error) => {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()).into_response()
        }
    }
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
pub async fn get_project(State(pool): State<PgPool>, Path(id): Path<String>) -> Response {
    if Uuid::parse_str(&id).is_err() {
        return error_response(StatusCode::NOT_FOUND, "project not found");
    }

    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id::text, org_id::text, name, slug, description, created_at::text, \
         updated_at::text FROM projects WHERE id = $1::uuid",
    )
    .bind(Uuid::parse_str(&id).ok())
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
