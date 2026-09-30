use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::identity::Caller;

#[derive(Serialize)]
pub struct CurrentUser {
    pub id: String,
    pub kind: &'static str,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub org_id: Option<String>,
    pub role: Option<String>,
}

pub async fn me(
    Extension(caller): Extension<Caller>,
    Extension(state): Extension<std::sync::Arc<super::state::AuthState>>,
) -> Response {
    let Some(user_id) = caller.user_id().and_then(|id| id.parse::<Uuid>().ok()) else {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "id": caller.header_value(), "kind": "service" })),
        )
            .into_response();
    };
    match state.service.describe(user_id).await {
        Some(user) => Json(user).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "the user no longer exists" })),
        )
            .into_response(),
    }
}
