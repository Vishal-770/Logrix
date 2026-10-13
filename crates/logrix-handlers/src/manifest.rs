use alloy_primitives::Address;
use logrix_core::domain::ChainId;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Root manifest configuration defining user indexing logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub chain_id: ChainId,
    pub contracts: Vec<ContractManifest>,
    #[serde(default)]
    pub webhooks: Vec<ManifestWebhook>,
    #[serde(default)]
    pub indexing: Option<ManifestIndexing>,
}

/// Operational and tuning options configured in manifest YAML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ManifestIndexing {
    pub ring_buffer_depth: Option<usize>,
    pub reconciler_interval_secs: Option<u64>,
    pub reconciler_chunk_size: Option<u64>,
    pub reconciler_lag_threshold: Option<u64>,
    pub chunk_size: Option<u64>,
    pub chain_head_poll_ms: Option<u64>,
    pub max_queue_depth: Option<u64>,
    pub wasm_fuel_limit: Option<u64>,
    pub wasm_max_memory_mb: Option<usize>,
}

/// Declarative webhook subscription from manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestWebhook {
    pub url: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

impl Manifest {
    /// Parse manifest from YAML string.
    pub fn from_yaml_str(yaml_str: &str) -> LogrixResult<Self> {
        serde_yaml::from_str(yaml_str).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Config,
                format!("Failed to parse YAML manifest: {e}"),
            )
        })
    }

    /// Read and parse manifest from filesystem.
    pub fn from_file<P: AsRef<Path>>(path: P) -> LogrixResult<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Config,
                format!("Failed to read manifest file {:?}: {e}", path.as_ref()),
            )
        })?;
        Self::from_yaml_str(&content)
    }
}

/// Indexed smart contract declaration and associated mapping handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractManifest {
    pub name: String,
    pub address: Address,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub declarative: Vec<DeclarativeRule>,
    pub wasm_handler: Option<String>,
    /// User toggle to disable indexing for this contract without removing config.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Per-contract execution fuel budget override.
    #[serde(default)]
    pub fuel_limit: Option<u64>,
    /// Per-contract memory ceiling override in megabytes.
    #[serde(default)]
    pub max_memory_mb: Option<usize>,
}

impl ContractManifest {
    /// Returns true if indexing is active for this contract.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

/// Declarative event mapping rule converting logs directly into entity records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarativeRule {
    /// Event signature name or topic0 hash
    pub event: String,
    /// Destination entity collection
    pub entity: String,
    /// Target entity property to log field extraction expression
    pub fields: HashMap<String, String>,
}
