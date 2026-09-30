use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: String,
}

const MAX_NAME_LENGTH: usize = 120;

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn server_error(error: sqlx::Error) -> Response {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()).into_response()
}

fn valid_slug(slug: &str) -> bool {
    !slug.trim().is_empty()
        && slug
            .trim()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[utoipa::path(
    get,
    path = "/api/v1/orgs",
    tag = "orgs",
    responses(
        (status = 200, description = "List all orgs", body = Vec<OrgResponse>)
    )
)]
pub async fn list_orgs(State(pool): State<PgPool>) -> Response {
    let orgs = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id::text, name, slug FROM orgs ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await;

    match orgs {
        Ok(rows) => {
            let orgs: Vec<OrgResponse> = rows
                .into_iter()
                .map(|(id, name, slug)| OrgResponse { id, name, slug })
                .collect();
            (StatusCode::OK, Json(orgs)).into_response()
        }
        Err(error) => server_error(error),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/orgs",
    tag = "orgs",
    request_body = CreateOrgRequest,
    responses(
        (status = 201, description = "Org created", body = OrgResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Slug already used")
    )
)]
pub async fn create_org(
    State(pool): State<PgPool>,
    Json(body): Json<CreateOrgRequest>,
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
    if !valid_slug(&body.slug) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "slug may only contain lowercase letters, digits and hyphens",
        );
    }

    let row = sqlx::query_as::<_, (String, String, String)>(
        "INSERT INTO orgs (name, slug) VALUES ($1, $2) \
         RETURNING id::text, name, slug",
    )
    .bind(body.name.trim())
    .bind(body.slug.trim().to_lowercase())
    .fetch_one(&pool)
    .await;

    match row {
        Ok(row) => (
            StatusCode::CREATED,
            Json(OrgResponse {
                id: row.0,
                name: row.1,
                slug: row.2,
            }),
        )
            .into_response(),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            error_response(StatusCode::CONFLICT, "an org with this slug already exists")
                .into_response()
        }
        Err(error) => server_error(error),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/orgs/{id}",
    tag = "orgs",
    params(("id" = String, Path, description = "Org ID")),
    responses(
        (status = 200, description = "Org details", body = OrgResponse),
        (status = 404, description = "Org not found")
    )
)]
pub async fn get_org(State(pool): State<PgPool>, Path(id): Path<String>) -> Response {
    let Ok(parsed) = Uuid::parse_str(&id) else {
        return error_response(StatusCode::NOT_FOUND, "org not found");
    };

    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id::text, name, slug FROM orgs WHERE id = $1::uuid",
    )
    .bind(parsed)
    .fetch_optional(&pool)
    .await;

    match row {
        Ok(Some(row)) => (
            StatusCode::OK,
            Json(OrgResponse {
                id: row.0,
                name: row.1,
                slug: row.2,
            }),
        )
            .into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "org not found").into_response(),
        Err(error) => server_error(error),
    }
}
