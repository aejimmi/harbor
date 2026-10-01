use serde::Deserialize;

/// Specification for a single server to create.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    pub location: String,
    pub image: String,
}
