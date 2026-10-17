use serde::{Deserialize, Serialize};

pub const DEFAULT_LEASE_DURATION_SECONDS: i32 = 15;

fn default_api_version() -> String {
    "coordination.k8s.io/v1".to_string()
}

fn default_kind() -> String {
    "Lease".to_string()
}

fn default_lease_duration() -> i32 {
    DEFAULT_LEASE_DURATION_SECONDS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseSpec {
    #[serde(rename = "holderIdentity", skip_serializing_if = "Option::is_none")]
    pub holder_identity: Option<String>,
    #[serde(rename = "leaseDurationSeconds", default = "default_lease_duration")]
    pub lease_duration_seconds: i32,
    #[serde(rename = "acquireTime", skip_serializing_if = "Option::is_none")]
    pub acquire_time: Option<String>,
    #[serde(rename = "renewTime", skip_serializing_if = "Option::is_none")]
    pub renew_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseMetadata {
    pub name: String,
    pub namespace: String,
    #[serde(rename = "resourceVersion", skip_serializing_if = "Option::is_none")]
    pub resource_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseObject {
    #[serde(rename = "apiVersion", default = "default_api_version")]
    pub api_version: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub metadata: LeaseMetadata,
    pub spec: LeaseSpec,
}

impl LeaseObject {
    pub fn new(metadata: LeaseMetadata, spec: LeaseSpec) -> Self {
        Self {
            api_version: default_api_version(),
            kind: default_kind(),
            metadata,
            spec,
        }
    }
}
