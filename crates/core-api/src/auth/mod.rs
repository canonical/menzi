pub mod clock;
pub mod config;
pub mod cookie;
pub mod handlers;
pub mod identity;
pub mod mailer;
pub mod middleware;
pub mod oidc;
pub mod service;
pub mod session;
pub mod state;
pub mod stores;

#[cfg(test)]
mod tests;

pub use config::{AuthConfig, RegistrationMode};
pub use service::AuthService;
pub use state::{in_memory_stores, postgres_stores, AuthState, Stores};
