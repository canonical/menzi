use menzi_db::Database;
use sqlx::Connection;

fn split_admin(admin: &str) -> (String, String) {
    let idx = admin
        .rfind('/')
        .expect("database url must include a database name");
    let base = admin[..idx].to_string();
    let db = admin[idx + 1..].to_string();
    (base, db)
}

async fn fresh_database_url(admin_url: &str) -> (String, String) {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (base, _) = split_admin(admin_url);
    let name = format!("menzi_it_{}_{}", std::process::id(), n);
    let mut admin = sqlx::PgConnection::connect(admin_url)
        .await
        .expect("connect to admin database");
    sqlx::query(&format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"))
        .execute(&mut admin)
        .await
        .expect("drop leftover database");
    sqlx::query(&format!("CREATE DATABASE \"{name}\""))
        .execute(&mut admin)
        .await
        .expect("create test database");
    (format!("{base}/{name}"), name)
}

async fn cleanup_database(admin_url: &str, name: &str) {
    let mut admin = sqlx::PgConnection::connect(admin_url)
        .await
        .expect("connect to admin database");
    sqlx::query(&format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"))
        .execute(&mut admin)
        .await
        .expect("drop test database");
}

const MIGRATION_COUNT: i64 = 8;

const EXPECTED_TABLES: &[&str] = &[
    "users",
    "projects",
    "project_members",
    "workspaces",
    "environments",
    "previews",
    "design_revisions",
    "design_clauses",
    "design_amendments",
    "design_waivers",
    "design_baselines",
    "design_conformance_reports",
    "design_discussions",
    "llm_credentials",
    "llm_usage",
    "audit_logs",
    "git_repos",
    "git_prs",
    "notifications",
    "notification_preferences",
    "web_push_subscriptions",
    "jobs",
    "templates",
];

#[tokio::test]
async fn database_connect_applies_all_migrations() {
    let Some(admin_url) = std::env::var("MENZI_TEST_DATABASE_URL").ok() else {
        eprintln!("skipping: MENZI_TEST_DATABASE_URL not set");
        return;
    };
    let (url, name) = fresh_database_url(&admin_url).await;
    let db = Database::connect(&url)
        .await
        .expect("database connect runs migrations");

    db.health_check().await.expect("health check after connect");

    let applied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success = true")
            .fetch_one(db.pool())
            .await
            .expect("query applied migrations");
    assert_eq!(applied, MIGRATION_COUNT);

    cleanup_database(&admin_url, &name).await;
}

#[tokio::test]
async fn database_connect_creates_expected_schema() {
    let Some(admin_url) = std::env::var("MENZI_TEST_DATABASE_URL").ok() else {
        eprintln!("skipping: MENZI_TEST_DATABASE_URL not set");
        return;
    };
    let (url, name) = fresh_database_url(&admin_url).await;
    let db = Database::connect(&url)
        .await
        .expect("database connect runs migrations");

    for table in EXPECTED_TABLES {
        let exists: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(table)
            .fetch_one(db.pool())
            .await
            .expect("query table presence");
        assert!(exists.is_some(), "expected table {table} to exist");
    }

    cleanup_database(&admin_url, &name).await;
}

#[tokio::test]
async fn database_health_check_passes_on_connected_database() {
    let Some(admin_url) = std::env::var("MENZI_TEST_DATABASE_URL").ok() else {
        eprintln!("skipping: MENZI_TEST_DATABASE_URL not set");
        return;
    };
    let (url, name) = fresh_database_url(&admin_url).await;
    let db = Database::connect(&url)
        .await
        .expect("database connect runs migrations");

    db.health_check().await.expect("health check succeeds");

    cleanup_database(&admin_url, &name).await;
}
