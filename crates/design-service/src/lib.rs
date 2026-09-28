pub mod impact;
pub mod mcp_tools;
pub mod types;

pub use types::*;

pub fn create_router() -> axum::Router {
    axum::Router::new()
}
