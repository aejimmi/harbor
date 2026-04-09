pub mod hetzner;

#[cfg(test)]
mod provider_test;

use std::net::IpAddr;

use async_trait::async_trait;

use crate::config::ServerSpec;

/// A server managed by a cloud provider.
#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)]
pub struct Server {
    pub id: i64,
    pub name: String,
    pub status: ServerStatus,
    pub ip: Option<IpAddr>,
    pub server_type: String,
    pub location: String,
}

/// Status of a cloud server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Initializing,
    Running,
    Off,
    Starting,
    Stopping,
    Deleting,
    Migrating,
    Rebuilding,
    Unknown,
}

/// Errors from cloud provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("failed to create server '{name}': {source}")]
    CreateFailed { name: String, source: anyhow::Error },

    #[error("server '{name}' timed out waiting for running status")]
    Timeout { name: String },

    #[error("SSH key '{name}' not found")]
    SshKeyNotFound { name: String },

    #[error("API error: {0}")]
    Api(#[from] anyhow::Error),
}

/// Abstraction over cloud server providers.
#[async_trait]
pub trait CloudProvider: Send + Sync {
    /// Create a server and wait until it reaches running status.
    async fn create_server(
        &self,
        spec: &ServerSpec,
        ssh_key: &str,
    ) -> Result<Server, ProviderError>;

    /// Delete a server by name. Idempotent — returns `Ok(())` if not found.
    async fn delete_server(&self, name: &str) -> Result<(), ProviderError>;

    /// List all servers.
    async fn list_servers(&self) -> Result<Vec<Server>, ProviderError>;

    /// Get a server by name. Returns `None` if not found.
    async fn get_server(&self, name: &str) -> Result<Option<Server>, ProviderError>;
}
