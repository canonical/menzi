use axum::{extract::Path, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

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

#[utoipa::path(
    get,
    path = "/api/v1/orgs",
    tag = "orgs",
    responses(
        (status = 200, description = "List all orgs", body = Vec<OrgResponse>)
    )
)]
pub async fn list_orgs(pool: axum::extract::State<PgPool>) -> Json<Vec<OrgResponse>> {
    let orgs = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, slug FROM orgs ORDER BY created_at DESC",
    )
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    Json(
        orgs.into_iter()
            .map(|(id, name, slug)| OrgResponse { id, name, slug })
            .collect(),
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/orgs",
    tag = "orgs",
    request_body = CreateOrgRequest,
    responses(
        (status = 201, description = "Org created", body = OrgResponse)
    )
)]
pub async fn create_org(
    pool: axum::extract::State<PgPool>,
    Json(body): Json<CreateOrgRequest>,
) -> Json<OrgResponse> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "INSERT INTO orgs (name, slug) VALUES ($1, $2) RETURNING id, name, slug",
    )
    .bind(&body.name)
    .bind(&body.slug)
    .fetch_one(&*pool)
    .await
    .unwrap_or_else(|_| {
        (
            "placeholder".to_string(),
            "placeholder".to_string(),
            "placeholder".to_string(),
        )
    });

    Json(OrgResponse {
        id: row.0,
        name: row.1,
        slug: row.2,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/orgs/{id}",
    tag = "orgs",
    params(
        ("id" = String, Path, description = "Org ID")
    ),
    responses(
        (status = 200, description = "Org details", body = OrgResponse)
    )
)]
pub async fn get_org(
    pool: axum::extract::State<PgPool>,
    Path(id): Path<String>,
) -> Json<OrgResponse> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, slug FROM orgs WHERE id = $1",
    )
    .bind(&id)
    .fetch_one(&*pool)
    .await
    .unwrap_or_else(|_| {
        (
            "placeholder".to_string(),
            "placeholder".to_string(),
            "placeholder".to_string(),
        )
    });

    Json(OrgResponse {
        id: row.0,
        name: row.1,
        slug: row.2,
    })
}
