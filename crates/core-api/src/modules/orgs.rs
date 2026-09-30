use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::identity::Caller;

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

pub fn caller_uuid(caller: &Caller) -> Option<Uuid> {
    caller.user_id().and_then(|id| Uuid::parse_str(id).ok())
}

#[utoipa::path(
    get,
    path = "/api/v1/orgs",
    tag = "orgs",
    responses(
        (status = 200, description = "List the caller's orgs", body = Vec<OrgResponse>)
    )
)]
pub async fn list_orgs(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
) -> Response {
    let Some(caller_id) = caller_uuid(&caller) else {
        return (StatusCode::OK, Json(Vec::<OrgResponse>::new())).into_response();
    };

    let orgs = sqlx::query_as::<_, (String, String, String)>(
        "SELECT o.id::text, o.name, o.slug
         FROM orgs o
         LEFT JOIN users u ON u.org_id = o.id
         WHERE u.id = $1::uuid
         ORDER BY o.created_at DESC",
    )
    .bind(caller_id)
    .fetch_all(&pool)
    .await;

    match orgs {
        Ok(rows) => {
            let orgs: Vec<OrgResponse> = rows
                .into_iter()
                .map(|row| OrgResponse {
                    id: row.0,
                    name: row.1,
                    slug: row.2,
                })
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
        (status = 401, description = "No caller"),
        (status = 409, description = "Slug already used")
    )
)]
pub async fn create_org(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Json(body): Json<CreateOrgRequest>,
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
    if !valid_slug(&body.slug) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "slug may only contain lowercase letters, digits and hyphens",
        );
    }

    let inserted = sqlx::query_as::<_, (Uuid, String, String)>(
        "INSERT INTO orgs (name, slug) VALUES ($1, $2) \
         RETURNING id, name, slug",
    )
    .bind(body.name.trim())
    .bind(body.slug.trim().to_lowercase())
    .fetch_one(&pool)
    .await;

    let row = match inserted {
        Ok(row) => row,
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            return error_response(StatusCode::CONFLICT, "an org with this slug already exists");
        }
        Err(error) => return server_error(error),
    };

    // The first org a user creates becomes theirs, so a new account has somewhere
    // to put its first project. It is only ever set once.
    if let Err(error) = sqlx::query("UPDATE users SET org_id = $2 WHERE id = $1 AND org_id IS NULL")
        .bind(caller_id)
        .bind(row.0)
        .execute(&pool)
        .await
    {
        tracing::warn!("could not attach the new org to its creator: {error}");
    }

    (
        StatusCode::CREATED,
        Json(OrgResponse {
            id: row.0.to_string(),
            name: row.1,
            slug: row.2,
        }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/orgs/{id}",
    tag = "orgs",
    params(("id" = String, Path, description = "Org ID")),
    responses(
        (status = 200, description = "Org details", body = OrgResponse),
        (status = 403, description = "Not the caller's org"),
        (status = 404, description = "Org not found")
    )
)]
pub async fn get_org(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<String>,
) -> Response {
    let Ok(parsed) = Uuid::parse_str(&id) else {
        return error_response(StatusCode::NOT_FOUND, "org not found");
    };
    let Some(caller_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::FORBIDDEN, "not your organisation");
    };

    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT o.id::text, o.name, o.slug
         FROM orgs o
         JOIN users u ON u.org_id = o.id
         WHERE o.id = $1::uuid AND u.id = $2::uuid",
    )
    .bind(parsed)
    .bind(caller_id)
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
        Ok(None) => error_response(StatusCode::NOT_FOUND, "org not found"),
        Err(error) => server_error(error),
    }
}
