pub mod dispatch;
pub mod preferences;
pub mod types;

pub use types::*;

pub fn create_router() -> axum::Router {
    axum::Router::new()
}
