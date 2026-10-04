pub mod api;
pub mod credentials;
pub mod driver;
pub mod gateway;
pub mod identity;
pub mod manager;
pub mod postgres;
pub mod registry;
pub mod store;
pub mod types;

pub use credentials::{OpencodeAuth, WorkspaceCredentialManager};
pub use driver::{LxdWorkspaceDriver, ProvisionOutcome, WorkspaceDriver};
pub use gateway::{HttpOpencodeGateway, OpencodeGateway};
pub use identity::{Authorizer, MembershipAuthorizer, PermissiveAuthorizer, Principal};
pub use manager::{WorkspaceAudit, WorkspaceManager};
pub use postgres::PostgresWorkspaceStore;
pub use registry::{PostgresSessionRegistry, SessionBinding, SessionKind, SessionRegistry};
pub use store::{InMemoryWorkspaceStore, WorkspaceStore};
pub use types::{
    AgentSession, PromptOutcome, ReconcileReport, TerminalChunk, TerminalInputSpec,
    TerminalRequest, TerminalResizeSpec, TerminalResult, Workspace, WorkspaceKey, WorkspaceSpec,
    WorkspaceStatus, WorkspaceTerminalOutput, WorkspaceTerminalSnapshot, WorkspaceTerminalStatus,
};
