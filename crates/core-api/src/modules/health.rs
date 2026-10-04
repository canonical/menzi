use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Serialize, ToSchema)]
pub struct ReadyResponse {
    pub status: String,
    pub database: String,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/ready",
    tag = "health",
    responses(
        (status = 200, description = "Service is ready", body = ReadyResponse),
        (status = 503, description = "Service is not ready", body = ReadyResponse)
    )
)]
pub async fn ready_check(State(pool): State<PgPool>) -> (StatusCode, Json<ReadyResponse>) {
    match tokio::time::timeout(
        std::time::Duration::from_millis(750),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&pool),
    )
    .await
    {
        Ok(Ok(_)) => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready".to_string(),
                database: "ok".to_string(),
            }),
        ),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "degraded".to_string(),
                database: "unavailable".to_string(),
            }),
        ),
    }
}
