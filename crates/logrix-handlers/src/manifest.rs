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
#[serde(rename_all = "camelCase")]
pub struct ManifestIndexing {
    #[serde(alias = "ring_buffer_depth")]
    pub ring_buffer_depth: Option<usize>,
    #[serde(alias = "reconciler_interval_secs")]
    pub reconciler_interval_secs: Option<u64>,
    #[serde(alias = "reconciler_chunk_size")]
    pub reconciler_chunk_size: Option<u64>,
    #[serde(alias = "reconciler_lag_threshold")]
    pub reconciler_lag_threshold: Option<u64>,
    #[serde(alias = "chunk_size")]
    pub chunk_size: Option<u64>,
    #[serde(alias = "chain_head_poll_ms")]
    pub chain_head_poll_ms: Option<u64>,
    #[serde(alias = "max_queue_depth")]
    pub max_queue_depth: Option<u64>,
    #[serde(alias = "wasm_fuel_limit")]
    pub wasm_fuel_limit: Option<u64>,
    #[serde(alias = "wasm_max_memory_mb")]
    pub wasm_max_memory_mb: Option<usize>,
}

/// Declarative webhook subscription from manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWebhook {
    pub url: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default, alias = "max_retries")]
    pub max_retries: Option<u32>,
}

impl Manifest {
    /// Parse manifest from YAML string (supports both runtime and SDK project formats).
    pub fn from_yaml_str(yaml_str: &str) -> LogrixResult<Self> {
        let val: serde_yaml::Value = serde_yaml::from_str(yaml_str).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Config,
                format!("Failed to parse YAML manifest: {e}"),
            )
        })?;

        // 1. Try standard runtime format (has "contracts" or "schema_version")
        if val.get("contracts").is_some() || val.get("schema_version").is_some() {
            return serde_yaml::from_value::<Manifest>(val).map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Config,
                    format!("Failed to deserialize runtime manifest: {e}"),
                )
            });
        }

        // 2. Try SDK project format (has "specVersion" or "dataSources")
        if val.get("specVersion").is_some() || val.get("dataSources").is_some() {
            let sdk = serde_yaml::from_value::<crate::sdk_manifest::SdkProjectManifest>(val)
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Permanent,
                        ErrorSource::Config,
                        format!("Failed to deserialize SDK project manifest: {e}"),
                    )
                })?;
            return Ok(sdk.into_manifest());
        }

        // 3. Fallback direct deserialization
        serde_yaml::from_value(val).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Config,
                format!("Unsupported manifest format: {e}"),
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
