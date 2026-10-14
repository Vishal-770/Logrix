use super::{ContractManifest, ManifestIndexing, ManifestWebhook};
use alloy_primitives::Address;
use logrix_core::domain::ChainId;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct SdkProjectManifest {
    #[serde(rename = "specVersion")]
    pub spec_version: String,
    pub network: SdkNetwork,
    #[serde(rename = "dataSources", default)]
    pub data_sources: Vec<SdkDataSource>,
    #[serde(default)]
    pub webhooks: Vec<ManifestWebhook>,
    #[serde(default)]
    pub indexing: Option<ManifestIndexing>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SdkNetwork {
    #[serde(rename = "chainId")]
    pub chain_id: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SdkDataSource {
    pub name: String,
    pub source: SdkSource,
    pub mapping: SdkMapping,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SdkSource {
    pub address: Address,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SdkMapping {
    #[serde(default = "default_wasm_file")]
    pub file: String,
    #[serde(rename = "eventHandlers", default)]
    pub event_handlers: Vec<SdkEventHandler>,
    #[serde(rename = "fuelLimit")]
    pub fuel_limit: Option<u64>,
    #[serde(rename = "maxMemoryMb")]
    pub max_memory_mb: Option<usize>,
    pub enabled: Option<bool>,
}

fn default_wasm_file() -> String {
    "./build/mapping.wasm".to_string()
}

#[derive(Debug, Deserialize)]
pub(crate) struct SdkEventHandler {
    pub event: String,
}

impl SdkProjectManifest {
    pub(crate) fn into_manifest(self) -> super::Manifest {
        let chain_id = ChainId::new(self.network.chain_id);
        let contracts = self
            .data_sources
            .into_iter()
            .map(|ds| {
                let events = ds
                    .mapping
                    .event_handlers
                    .into_iter()
                    .map(|eh| eh.event)
                    .collect();
                ContractManifest {
                    name: ds.name,
                    address: ds.source.address,
                    events,
                    declarative: Vec::new(),
                    wasm_handler: Some(ds.mapping.file),
                    enabled: ds.mapping.enabled,
                    fuel_limit: ds.mapping.fuel_limit,
                    max_memory_mb: ds.mapping.max_memory_mb,
                }
            })
            .collect();

        super::Manifest {
            schema_version: self.spec_version,
            chain_id,
            contracts,
            webhooks: self.webhooks,
            indexing: self.indexing,
        }
    }
}
