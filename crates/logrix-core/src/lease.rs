use crate::error::LogrixResult;
use crate::ports::LeaderElectionPort;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Kubernetes Lease representation (coordination.k8s.io/v1).
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

fn default_lease_duration() -> i32 {
    15
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseObject {
    pub metadata: LeaseMetadata,
    pub spec: LeaseSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseMetadata {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_version: Option<String>,
}

/// Native Kubernetes Lease Leader Elector communicating directly with K8s API server.
pub struct KubernetesLeaseElector {
    client: reqwest::Client,
    k8s_api_url: String,
    namespace: String,
    lease_name: String,
    identity: String,
    token: Option<String>,
    is_leader: AtomicBool,
    last_resource_version: Arc<RwLock<Option<String>>>,
}

impl KubernetesLeaseElector {
    /// Create new Kubernetes Lease leader elector from environment or parameters.
    pub fn new(
        lease_name: impl Into<String>,
        namespace: impl Into<String>,
        identity: impl Into<String>,
        k8s_api_url: Option<String>,
        token: Option<String>,
    ) -> Self {
        let host = std::env::var("KUBERNETES_SERVICE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("KUBERNETES_SERVICE_PORT").unwrap_or_else(|_| "443".into());
        let default_url = format!("https://{host}:{port}");
        let resolved_token = token
            .or_else(|| std::env::var("KUBERNETES_BEARER_TOKEN").ok())
            .or_else(|| {
                std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/token").ok()
            });
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_default();

        Self {
            client,
            k8s_api_url: k8s_api_url.unwrap_or(default_url),
            namespace: namespace.into(),
            lease_name: lease_name.into(),
            identity: identity.into(),
            token: resolved_token,
            is_leader: AtomicBool::new(false),
            last_resource_version: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait]
impl LeaderElectionPort for KubernetesLeaseElector {
    async fn try_acquire_or_renew(&self) -> LogrixResult<bool> {
        let url = format!(
            "{}/apis/coordination.k8s.io/v1/namespaces/{}/leases/{}",
            self.k8s_api_url, self.namespace, self.lease_name
        );

        let mut req = self.client.get(&url);
        if let Some(ref t) = self.token {
            req = req.bearer_auth(t);
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "Failed to connect to K8s API server for Lease check; falling back to non-leader");
                self.is_leader.store(false, Ordering::SeqCst);
                return Ok(false);
            }
        };

        if resp.status().is_success() {
            if let Ok(lease) = resp.json::<LeaseObject>().await {
                let mut rv = self.last_resource_version.write().await;
                *rv = lease.metadata.resource_version.clone();
                let current_holder = lease.spec.holder_identity.as_deref().unwrap_or("");
                let is_current = current_holder == self.identity;
                self.is_leader.store(is_current, Ordering::SeqCst);
                return Ok(is_current);
            }
        }

        info!(identity = %self.identity, "Acquiring or assuming leader role");
        self.is_leader.store(true, Ordering::SeqCst);
        Ok(true)
    }

    async fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    async fn step_down(&self) -> LogrixResult<()> {
        info!(identity = %self.identity, "Stepping down from Kubernetes Lease leader role");
        self.is_leader.store(false, Ordering::SeqCst);
        Ok(())
    }
}
