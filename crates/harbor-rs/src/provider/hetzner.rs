use std::net::IpAddr;
use std::time::Duration;

use hcloud::apis::configuration::Configuration;
use hcloud::apis::{servers_api, ssh_keys_api};
use hcloud::models;

use super::{ProviderError, Server, ServerStatus};
use crate::config::ServerSpec;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_POLL_ATTEMPTS: u32 = 60;

/// Hetzner Cloud provider implementation.
pub struct HetznerProvider {
    config: Configuration,
}

impl HetznerProvider {
    /// Create a new provider with the given API token.
    pub fn new(token: &str) -> Self {
        let mut config = Configuration::new();
        config.bearer_access_token = Some(token.to_owned());
        Self { config }
    }
}

#[async_trait::async_trait]
impl super::CloudProvider for HetznerProvider {
    async fn create_server(
        &self,
        spec: &ServerSpec,
        ssh_key: &str,
    ) -> Result<Server, ProviderError> {
        // Check if server already exists
        if let Some(existing) = self.get_server(&spec.name).await?
            && existing.status == ServerStatus::Running
        {
            return Ok(existing);
        }

        // Look up SSH key by name to validate it exists
        let keys_resp = ssh_keys_api::list_ssh_keys(
            &self.config,
            ssh_keys_api::ListSshKeysParams {
                name: Some(ssh_key.to_owned()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| ProviderError::Api(anyhow::anyhow!("{e}")))?;

        if keys_resp.ssh_keys.is_empty() {
            return Err(ProviderError::SshKeyNotFound {
                name: ssh_key.to_owned(),
            });
        }

        // Create server
        let mut request = models::CreateServerRequest::new(
            spec.image.clone(),
            spec.name.clone(),
            spec.server_type.clone(),
        );
        request.location = Some(spec.location.clone());
        request.ssh_keys = Some(vec![ssh_key.to_owned()]);

        let create_resp = servers_api::create_server(
            &self.config,
            servers_api::CreateServerParams {
                create_server_request: request,
            },
        )
        .await
        .map_err(|e| ProviderError::CreateFailed {
            name: spec.name.clone(),
            source: anyhow::anyhow!("{e}"),
        })?;

        let server_id = create_resp.server.id;
        tracing::info!(id = server_id, name = %spec.name, "server created, waiting for running status");

        // Poll until running
        self.wait_for_running(server_id, &spec.name).await
    }

    async fn delete_server(&self, name: &str) -> Result<(), ProviderError> {
        let Some(server) = self.get_server(name).await? else {
            return Ok(());
        };

        servers_api::delete_server(
            &self.config,
            servers_api::DeleteServerParams { id: server.id },
        )
        .await
        .map_err(|e| ProviderError::Api(anyhow::anyhow!("{e}")))?;

        Ok(())
    }

    async fn list_servers(&self) -> Result<Vec<Server>, ProviderError> {
        let resp = servers_api::list_servers(
            &self.config,
            servers_api::ListServersParams {
                per_page: Some(50),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| ProviderError::Api(anyhow::anyhow!("{e}")))?;

        Ok(resp
            .servers
            .into_iter()
            .map(|s| convert_server(&s))
            .collect())
    }

    async fn get_server(&self, name: &str) -> Result<Option<Server>, ProviderError> {
        let resp = servers_api::list_servers(
            &self.config,
            servers_api::ListServersParams {
                name: Some(name.to_owned()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| ProviderError::Api(anyhow::anyhow!("{e}")))?;

        Ok(resp.servers.first().map(convert_server))
    }
}

impl HetznerProvider {
    async fn wait_for_running(&self, server_id: i64, name: &str) -> Result<Server, ProviderError> {
        for attempt in 1..=MAX_POLL_ATTEMPTS {
            tokio::time::sleep(POLL_INTERVAL).await;

            let resp = servers_api::get_server(
                &self.config,
                servers_api::GetServerParams { id: server_id },
            )
            .await
            .map_err(|e| ProviderError::Api(anyhow::anyhow!("{e}")))?;

            let hcloud_server = resp.server.as_deref().ok_or_else(|| {
                ProviderError::Api(anyhow::anyhow!("get_server returned no server"))
            })?;
            let server = convert_server(hcloud_server);
            if server.status == ServerStatus::Running {
                tracing::info!(id = server_id, attempt, "server is running");
                return Ok(server);
            }

            tracing::debug!(id = server_id, attempt, status = ?server.status, "waiting...");
        }

        Err(ProviderError::Timeout {
            name: name.to_owned(),
        })
    }
}

fn convert_server(s: &models::Server) -> Server {
    let ip = s
        .public_net
        .ipv4
        .as_ref()
        .and_then(|v4| v4.ip.parse::<IpAddr>().ok());

    let status = match s.status {
        models::server::Status::Running => ServerStatus::Running,
        models::server::Status::Off => ServerStatus::Off,
        models::server::Status::Initializing => ServerStatus::Initializing,
        models::server::Status::Starting => ServerStatus::Starting,
        models::server::Status::Stopping => ServerStatus::Stopping,
        models::server::Status::Deleting => ServerStatus::Deleting,
        models::server::Status::Migrating => ServerStatus::Migrating,
        models::server::Status::Rebuilding => ServerStatus::Rebuilding,
        models::server::Status::Unknown => ServerStatus::Unknown,
    };

    let location = s.datacenter.location.name.clone();
    let server_type = s.server_type.name.clone();

    Server {
        id: s.id,
        name: s.name.clone(),
        status,
        ip,
        server_type,
        location,
    }
}
