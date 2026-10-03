use crate::error::LogrixResult;
use crate::ports::LeaderElectionPort;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// In-memory leader elector for local standalone execution and testing.
#[derive(Debug, Default)]
pub struct LocalLeaderElector {
    is_leader: AtomicBool,
}

impl LocalLeaderElector {
    /// Create new local leader elector (defaults to leader).
    pub fn new() -> Self {
        Self {
            is_leader: AtomicBool::new(true),
        }
    }
}

#[async_trait]
impl LeaderElectionPort for LocalLeaderElector {
    async fn try_acquire_or_renew(&self) -> LogrixResult<bool> {
        self.is_leader.store(true, Ordering::SeqCst);
        Ok(true)
    }

    async fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    async fn step_down(&self) -> LogrixResult<()> {
        self.is_leader.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// Kubernetes Lease representation (coordination.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseSpec {
    #[serde(rename = "holderIdentity", skip_serializing_if = "Option::is_none")]
    pub holder_identity: Option<String>,
    #[serde(rename = "leaseDurationSeconds", default = "default_duration")]
    pub lease_duration_seconds: i32,
    #[serde(rename = "acquireTime", skip_serializing_if = "Option::is_none")]
    pub acquire_time: Option<String>,
    #[serde(rename = "renewTime", skip_serializing_if = "Option::is_none")]
    pub renew_time: Option<String>,
}

fn default_duration() -> i32 {
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
            req = req.bearer_auth(t.trim());
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "Failed to connect to Kubernetes API server for lease check");
                self.is_leader.store(false, Ordering::SeqCst);
                return Ok(false);
            }
        };

        let now_str = chrono::Utc::now().to_rfc3339();

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            // Lease doesn't exist; create it
            let new_lease = LeaseObject {
                metadata: LeaseMetadata {
                    name: self.lease_name.clone(),
                    namespace: self.namespace.clone(),
                    resource_version: None,
                },
                spec: LeaseSpec {
                    holder_identity: Some(self.identity.clone()),
                    lease_duration_seconds: 15,
                    acquire_time: Some(now_str.clone()),
                    renew_time: Some(now_str),
                },
            };

            let post_url = format!(
                "{}/apis/coordination.k8s.io/v1/namespaces/{}/leases",
                self.k8s_api_url, self.namespace
            );
            let mut post_req = self.client.post(&post_url).json(&new_lease);
            if let Some(ref t) = self.token {
                post_req = post_req.bearer_auth(t.trim());
            }

            if let Ok(res) = post_req.send().await {
                if res.status().is_success() {
                    info!(identity = %self.identity, lease = %self.lease_name, "Acquired leadership via newly created Lease");
                    self.is_leader.store(true, Ordering::SeqCst);
                    return Ok(true);
                }
            }
            self.is_leader.store(false, Ordering::SeqCst);
            return Ok(false);
        }

        if let Ok(lease) = resp.json::<LeaseObject>().await {
            *self.last_resource_version.write().await = lease.metadata.resource_version.clone();

            if lease.spec.holder_identity.as_deref() == Some(&self.identity) {
                // Already leader; renew
                self.is_leader.store(true, Ordering::SeqCst);
                return Ok(true);
            }
        }

        self.is_leader.store(false, Ordering::SeqCst);
        Ok(false)
    }

    async fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    async fn step_down(&self) -> LogrixResult<()> {
        self.is_leader.store(false, Ordering::SeqCst);
        info!(identity = %self.identity, lease = %self.lease_name, "Stepped down from leadership");
        Ok(())
    }
}
