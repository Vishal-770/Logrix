use alloy_primitives::Address;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Root manifest configuration defining user indexing logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub chain_id: u64,
    pub contracts: Vec<ContractManifest>,
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
