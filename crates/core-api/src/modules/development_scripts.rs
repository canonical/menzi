use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::identity::{caller_uuid, Caller};

#[derive(Serialize)]
pub struct DevelopmentScriptResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub slug: String,
    pub relative_path: Option<String>,
    pub body: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateDevelopmentScriptRequest {
    pub name: String,
    pub slug: String,
    pub body: String,
    pub relative_path: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateDevelopmentScriptRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub body: Option<String>,
    pub relative_path: Option<String>,
}

type ScriptRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
);

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn map_row(row: ScriptRow) -> DevelopmentScriptResponse {
    DevelopmentScriptResponse {
        id: row.0,
        project_id: row.1,
        name: row.2,
        slug: row.3,
        relative_path: row.4,
        body: row.5,
        source: row.6,
        created_at: row.7,
        updated_at: row.8,
    }
}

async fn ensure_member(pool: &PgPool, user_id: Uuid, project_id: Uuid) -> Result<(), Response> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM project_members WHERE user_id = $1::uuid AND project_id = $2::uuid",
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await;
    match row {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(error_response(StatusCode::FORBIDDEN, "not your project")),
        Err(error) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &error.to_string(),
        )),
    }
}

pub async fn list_development_scripts(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path(project_id): Path<String>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let Ok(project_id) = Uuid::parse_str(&project_id) else {
        return error_response(StatusCode::NOT_FOUND, "project not found");
    };
    if let Err(response) = ensure_member(&pool, user_id, project_id).await {
        return response;
    }
    let rows = sqlx::query_as::<_, ScriptRow>(
        "SELECT id::text, project_id::text, name, slug, relative_path, body, source, created_at::text, updated_at::text
         FROM project_development_scripts
         WHERE project_id = $1::uuid
         ORDER BY updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await;
    match rows {
        Ok(rows) => Json(rows.into_iter().map(map_row).collect::<Vec<_>>()).into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn create_development_script(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path(project_id): Path<String>,
    Json(body): Json<CreateDevelopmentScriptRequest>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let Ok(project_id) = Uuid::parse_str(&project_id) else {
        return error_response(StatusCode::NOT_FOUND, "project not found");
    };
    if let Err(response) = ensure_member(&pool, user_id, project_id).await {
        return response;
    }
    if body.name.trim().is_empty() || body.slug.trim().is_empty() || body.body.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "name, slug and body are required");
    }
    let row = sqlx::query_as::<_, ScriptRow>(
        "INSERT INTO project_development_scripts
            (project_id, name, slug, relative_path, body, source, created_by)
         VALUES ($1::uuid, $2, $3, $4, $5, 'manual', $6::uuid)
         RETURNING id::text, project_id::text, name, slug, relative_path, body, source, created_at::text, updated_at::text",
    )
    .bind(project_id)
    .bind(body.name.trim())
    .bind(body.slug.trim())
    .bind(body.relative_path.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(body.body.as_str())
    .bind(user_id)
    .fetch_one(&pool)
    .await;
    match row {
        Ok(row) => (StatusCode::CREATED, Json(map_row(row))).into_response(),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            error_response(StatusCode::CONFLICT, "a script with this slug already exists")
        }
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn update_development_script(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path((project_id, script_id)): Path<(String, String)>,
    Json(body): Json<UpdateDevelopmentScriptRequest>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let Ok(project_id) = Uuid::parse_str(&project_id) else {
        return error_response(StatusCode::NOT_FOUND, "project not found");
    };
    let Ok(script_id) = Uuid::parse_str(&script_id) else {
        return error_response(StatusCode::NOT_FOUND, "script not found");
    };
    if let Err(response) = ensure_member(&pool, user_id, project_id).await {
        return response;
    }
    let row = sqlx::query_as::<_, ScriptRow>(
        "UPDATE project_development_scripts
         SET name = COALESCE($3, name),
             slug = COALESCE($4, slug),
             relative_path = COALESCE($5, relative_path),
             body = COALESCE($6, body),
             source = CASE WHEN $6 IS NULL AND $3 IS NULL AND $4 IS NULL AND $5 IS NULL THEN source ELSE 'manual' END,
             updated_at = now()
         WHERE project_id = $1::uuid AND id = $2::uuid
         RETURNING id::text, project_id::text, name, slug, relative_path, body, source, created_at::text, updated_at::text",
    )
    .bind(project_id)
    .bind(script_id)
    .bind(body.name.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(body.slug.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(
        body.relative_path
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
    )
    .bind(body.body.as_deref())
    .fetch_optional(&pool)
    .await;
    match row {
        Ok(Some(row)) => Json(map_row(row)).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "script not found"),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            error_response(StatusCode::CONFLICT, "a script with this slug already exists")
        }
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

pub async fn delete_development_script(
    State(pool): State<PgPool>,
    Extension(caller): Extension<Caller>,
    Path((project_id, script_id)): Path<(String, String)>,
) -> Response {
    let Some(user_id) = caller_uuid(&caller) else {
        return error_response(StatusCode::UNAUTHORIZED, "who is calling is not established");
    };
    let Ok(project_id) = Uuid::parse_str(&project_id) else {
        return error_response(StatusCode::NOT_FOUND, "project not found");
    };
    let Ok(script_id) = Uuid::parse_str(&script_id) else {
        return error_response(StatusCode::NOT_FOUND, "script not found");
    };
    if let Err(response) = ensure_member(&pool, user_id, project_id).await {
        return response;
    }
    let result = sqlx::query(
        "DELETE FROM project_development_scripts WHERE project_id = $1::uuid AND id = $2::uuid",
    )
    .bind(project_id)
    .bind(script_id)
    .execute(&pool)
    .await;
    match result {
        Ok(result) if result.rows_affected() > 0 => {
            (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Ok(_) => error_response(StatusCode::NOT_FOUND, "script not found"),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}
