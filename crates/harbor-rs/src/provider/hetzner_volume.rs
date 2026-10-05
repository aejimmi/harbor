//! Hetzner block volumes: find by name, create attached, or attach.

use std::time::Duration;

use hcloud::apis::volumes_api;
use hcloud::models;

use super::hetzner::HetznerProvider;
use super::{AttachedVolume, ProviderError, Server};
use crate::config::VolumeSpec;

const ACTION_POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_ACTION_POLLS: u32 = 90;
const API_BASE: &str = "https://api.hetzner.cloud/v1";

// Action-bearing responses are parsed here rather than via hcloud's models,
// whose `Action` requires an `error` key that the API omits when it is null.

/// `GET /actions/{id}` and `POST /volumes/{id}/actions/attach` body.
#[derive(Debug, serde::Deserialize)]
pub(super) struct ActionEnvelope {
    pub(super) action: ActionState,
}

/// `POST /volumes` body.
#[derive(Debug, serde::Deserialize)]
pub(super) struct CreatedVolume {
    pub(super) volume: CreatedVolumeInfo,
    pub(super) action: ActionState,
    #[serde(default)]
    pub(super) next_actions: Vec<ActionState>,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct CreatedVolumeInfo {
    pub(super) id: i64,
    pub(super) linux_device: String,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct ActionState {
    pub(super) id: i64,
    pub(super) command: String,
    pub(super) status: String,
    #[serde(default)]
    pub(super) error: Option<ActionError>,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct ActionError {
    pub(super) message: String,
}

fn api_err(e: impl std::fmt::Display) -> ProviderError {
    ProviderError::Api(anyhow::anyhow!("{e}"))
}

impl HetznerProvider {
    /// See [`super::CloudProvider::ensure_volume`].
    pub(super) async fn ensure_volume_impl(
        &self,
        spec: &VolumeSpec,
        server: &Server,
    ) -> Result<AttachedVolume, ProviderError> {
        match self.find_volume(&spec.name).await? {
            None => self.create_attached(spec, server).await,
            Some(vol) => self.reuse(spec, vol, server).await,
        }
    }

    /// See [`super::CloudProvider::check_volume`].
    pub(super) async fn check_volume_impl(
        &self,
        spec: &VolumeSpec,
        location: &str,
        server_id: Option<i64>,
    ) -> Result<(), ProviderError> {
        match self.find_volume(&spec.name).await? {
            None => Ok(()),
            Some(vol) => reuse_conflict(
                &vol.name,
                &vol.location.name,
                vol.server,
                location,
                server_id,
            )
            .map_or(Ok(()), |why| Err(api_err(why))),
        }
    }

    async fn find_volume(&self, name: &str) -> Result<Option<models::Volume>, ProviderError> {
        let resp = volumes_api::list_volumes(
            self.config(),
            volumes_api::ListVolumesParams {
                name: Some(name.to_owned()),
                ..Default::default()
            },
        )
        .await
        .map_err(api_err)?;
        Ok(resp.volumes.into_iter().next())
    }

    /// New volume, formatted by Hetzner, attached without automount —
    /// harbor's setup script owns the mount and the fstab entry.
    async fn create_attached(
        &self,
        spec: &VolumeSpec,
        server: &Server,
    ) -> Result<AttachedVolume, ProviderError> {
        let size = i32::try_from(spec.size).map_err(api_err)?;
        let mut request = models::CreateVolumeRequest::new(spec.name.clone(), size);
        request.server = Some(server.id);
        request.automount = Some(false);
        request.format = Some(spec.format.as_str().to_owned());

        let resp: CreatedVolume = self.api_json("/volumes", Some(&request)).await?;
        tracing::info!(volume = %spec.name, id = resp.volume.id, "volume created");

        self.wait_for_action(resp.action.id).await?;
        for next in &resp.next_actions {
            self.wait_for_action(next.id).await?;
        }
        Ok(AttachedVolume {
            name: spec.name.clone(),
            linux_device: resp.volume.linux_device.clone(),
            created: true,
        })
    }

    /// Existing volume: must share the server's location and be free
    /// or already attached to this server.
    async fn reuse(
        &self,
        spec: &VolumeSpec,
        vol: models::Volume,
        server: &Server,
    ) -> Result<AttachedVolume, ProviderError> {
        if let Some(why) = reuse_conflict(
            &vol.name,
            &vol.location.name,
            vol.server,
            &server.location,
            Some(server.id),
        ) {
            return Err(api_err(why));
        }
        if vol.server.is_none() {
            self.attach(vol.id, server.id).await?;
        }
        if f64::from(spec.size) > vol.size {
            tracing::warn!(
                volume = %vol.name,
                declared = spec.size,
                actual = vol.size,
                "existing volume is smaller than declared; harbor never resizes — use `hcloud volume resize`"
            );
        }
        Ok(AttachedVolume {
            name: vol.name,
            linux_device: vol.linux_device,
            created: false,
        })
    }

    async fn attach(&self, volume_id: i64, server_id: i64) -> Result<(), ProviderError> {
        let mut request = models::AttachVolumeToServerRequest::new(server_id);
        request.automount = Some(false);
        let resp: ActionEnvelope = self
            .api_json(
                &format!("/volumes/{volume_id}/actions/attach"),
                Some(&request),
            )
            .await?;
        self.wait_for_action(resp.action.id).await
    }

    /// Poll an action until it succeeds; an `error` status or a timeout
    /// fails the caller.
    async fn wait_for_action(&self, id: i64) -> Result<(), ProviderError> {
        for _ in 0..MAX_ACTION_POLLS {
            let action = self.get_action(id).await?;
            match action.status.as_str() {
                "success" => return Ok(()),
                "error" => {
                    let msg = action
                        .error
                        .map_or_else(|| "unknown error".to_owned(), |e| e.message);
                    return Err(api_err(format!(
                        "action {} ({}) failed: {msg}",
                        id, action.command
                    )));
                }
                _ => tokio::time::sleep(ACTION_POLL_INTERVAL).await,
            }
        }
        Err(api_err(format!("action {id} did not finish in time")))
    }

    async fn get_action(&self, id: i64) -> Result<ActionState, ProviderError> {
        let resp: ActionEnvelope = self
            .api_json::<_, ()>(&format!("/actions/{id}"), None)
            .await?;
        Ok(resp.action)
    }

    /// GET `path`, or POST `body` to it, with hcloud's client and token;
    /// parse the response as `T`.
    async fn api_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, ProviderError> {
        let cfg = self.config();
        let url = format!("{API_BASE}{path}");
        let (method, mut req) = match body {
            Some(body) => ("POST", cfg.client.post(url).json(body)),
            None => ("GET", cfg.client.get(url)),
        };
        if let Some(token) = &cfg.bearer_access_token {
            req = req.bearer_auth(token);
        }
        let resp = req.send().await.map_err(api_err)?;
        let status = resp.status();
        let text = resp.text().await.map_err(api_err)?;
        if !status.is_success() {
            return Err(api_err(format!("{method} {path}: HTTP {status}: {text}")));
        }
        serde_json::from_str(&text).map_err(api_err)
    }
}

/// Why an existing volume cannot serve a server in `location` with id
/// `server_id` (`None` before the server exists), or `None` if it can.
pub(super) fn reuse_conflict(
    name: &str,
    vol_location: &str,
    vol_server: Option<i64>,
    location: &str,
    server_id: Option<i64>,
) -> Option<String> {
    if vol_location != location {
        return Some(format!(
            "volume '{name}' is in {vol_location}, but the server is in {location} — volumes cannot move"
        ));
    }
    match vol_server {
        Some(id) if Some(id) != server_id => Some(format!(
            "volume '{name}' is attached to another server (id {id}) — detach it first"
        )),
        _ => None,
    }
}
