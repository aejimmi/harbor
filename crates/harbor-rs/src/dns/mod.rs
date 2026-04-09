pub mod cloudflare;

#[cfg(test)]
mod dns_test;

use std::net::IpAddr;

use async_trait::async_trait;

use crate::config::UserConfig;

/// Errors from DNS provider operations.
#[derive(Debug, thiserror::Error)]
pub enum DnsError {
    #[error("failed to create DNS record for '{hostname}': {source}")]
    CreateFailed {
        hostname: String,
        source: anyhow::Error,
    },

    #[error("DNS record not found: {hostname}")]
    NotFound { hostname: String },

    #[error("DNS API error: {0}")]
    Api(#[from] anyhow::Error),
}

/// Abstraction over DNS providers.
#[async_trait]
pub trait DnsProvider: Send + Sync {
    /// Create an A record.
    async fn create_a_record(&self, hostname: &str, ip: IpAddr) -> Result<(), DnsError>;

    /// Create or update an A record (upsert).
    async fn upsert_a_record(&self, hostname: &str, ip: IpAddr) -> Result<(), DnsError>;

    /// Delete an A record by hostname.
    async fn delete_a_record(&self, hostname: &str) -> Result<(), DnsError>;
}

/// Extract hostname from server name pattern.
///
/// Pattern: `service-hostname-env-location`
/// Example: `"collector-tergar-prod-nbg1"` -> `"tergar"`
pub fn extract_hostname(server_name: &str) -> &str {
    let mut parts = server_name.split('-').filter(|s| !s.is_empty());
    // Skip the first part (service name), return the second (hostname).
    parts.next();
    parts.next().unwrap_or(server_name)
}

/// Build full DNS hostname from name and base domain.
///
/// Example: `("tergar", ".i.usercanal.com")` -> `"tergar.i.usercanal.com"`
pub fn full_hostname(name: &str, base_domain: &str) -> String {
    format!("{name}{base_domain}")
}

/// Check if DNS is configured (both `api_token` and `zone_id` present).
pub fn is_configured(config: &UserConfig) -> bool {
    !config.cloudflare.api_token.is_empty() && !config.cloudflare.zone_id.is_empty()
}
