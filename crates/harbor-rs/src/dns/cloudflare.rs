use std::net::IpAddr;

use ::cloudflare::endpoints::dns::dns::{
    CreateDnsRecord, CreateDnsRecordParams, DeleteDnsRecord, DnsContent, ListDnsRecords,
    ListDnsRecordsParams, UpdateDnsRecord, UpdateDnsRecordParams,
};
use ::cloudflare::framework::Environment;
use ::cloudflare::framework::auth::Credentials;
use ::cloudflare::framework::client::ClientConfig;
use ::cloudflare::framework::client::async_api::Client;

use super::DnsError;
use crate::config::UserConfig;

/// Cloudflare DNS provider implementation.
pub struct CloudflareProvider {
    client: Client,
    zone_id: String,
}

impl CloudflareProvider {
    /// Create a new Cloudflare provider from API token and zone ID.
    pub fn new(api_token: &str, zone_id: &str) -> Result<Self, DnsError> {
        let credentials = Credentials::UserAuthToken {
            token: api_token.to_owned(),
        };
        let client = Client::new(
            credentials,
            ClientConfig::default(),
            Environment::Production,
        )
        .map_err(|e| DnsError::Api(anyhow::anyhow!("failed to create Cloudflare client: {e}")))?;

        Ok(Self {
            client,
            zone_id: zone_id.to_owned(),
        })
    }

    /// Create from `UserConfig` if Cloudflare credentials are present.
    /// Returns `None` if credentials are missing.
    pub fn from_config(config: &UserConfig) -> Result<Option<Self>, DnsError> {
        if !super::is_configured(config) {
            return Ok(None);
        }
        Self::new(&config.cloudflare.api_token, &config.cloudflare.zone_id).map(Some)
    }
}

#[async_trait::async_trait]
impl super::DnsProvider for CloudflareProvider {
    async fn create_a_record(&self, hostname: &str, ip: IpAddr) -> Result<(), DnsError> {
        let ipv4 = match ip {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(_) => {
                return Err(DnsError::Api(anyhow::anyhow!(
                    "only IPv4 A records are supported"
                )));
            }
        };

        self.client
            .request(&CreateDnsRecord {
                zone_identifier: &self.zone_id,
                params: CreateDnsRecordParams {
                    ttl: Some(300),
                    priority: None,
                    proxied: None,
                    name: hostname,
                    content: DnsContent::A { content: ipv4 },
                },
            })
            .await
            .map_err(|e| DnsError::CreateFailed {
                hostname: hostname.to_owned(),
                source: anyhow::anyhow!("{e}"),
            })?;

        Ok(())
    }

    async fn upsert_a_record(&self, hostname: &str, ip: IpAddr) -> Result<(), DnsError> {
        let ipv4 = match ip {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(_) => {
                return Err(DnsError::Api(anyhow::anyhow!(
                    "only IPv4 A records are supported"
                )));
            }
        };

        // List existing records
        let records = self
            .client
            .request(&ListDnsRecords {
                zone_identifier: &self.zone_id,
                params: ListDnsRecordsParams {
                    name: Some(hostname.to_owned()),
                    record_type: Some(DnsContent::A {
                        content: "0.0.0.0".parse().expect("valid ip"),
                    }),
                    ..Default::default()
                },
            })
            .await
            .map_err(|e| DnsError::Api(anyhow::anyhow!("{e}")))?;

        if let Some(existing) = records.result.first() {
            // Update existing record
            self.client
                .request(&UpdateDnsRecord {
                    zone_identifier: &self.zone_id,
                    identifier: &existing.id,
                    params: UpdateDnsRecordParams {
                        ttl: Some(300),
                        proxied: None,
                        name: hostname,
                        content: DnsContent::A { content: ipv4 },
                    },
                })
                .await
                .map_err(|e| DnsError::Api(anyhow::anyhow!("{e}")))?;
        } else {
            // Create new record
            self.create_a_record(hostname, ip).await?;
        }

        Ok(())
    }

    async fn delete_a_record(&self, hostname: &str) -> Result<(), DnsError> {
        let records = self
            .client
            .request(&ListDnsRecords {
                zone_identifier: &self.zone_id,
                params: ListDnsRecordsParams {
                    name: Some(hostname.to_owned()),
                    record_type: Some(DnsContent::A {
                        content: "0.0.0.0".parse().expect("valid ip"),
                    }),
                    ..Default::default()
                },
            })
            .await
            .map_err(|e| DnsError::Api(anyhow::anyhow!("{e}")))?;

        if records.result.is_empty() {
            return Err(DnsError::NotFound {
                hostname: hostname.to_owned(),
            });
        }

        for record in &records.result {
            self.client
                .request(&DeleteDnsRecord {
                    zone_identifier: &self.zone_id,
                    identifier: &record.id,
                })
                .await
                .map_err(|e| DnsError::Api(anyhow::anyhow!("{e}")))?;
        }

        Ok(())
    }
}
