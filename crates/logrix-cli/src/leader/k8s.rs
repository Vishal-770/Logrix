#[path = "operations.rs"]
mod operations;
#[path = "types.rs"]
mod types;

use async_trait::async_trait;
use logrix_core::{error::LogrixResult, ports::LeaderElectionPort};
use reqwest::{Client, RequestBuilder, StatusCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{info, warn};
use types::LeaseObject;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Kubernetes Lease adapter for the core leader-election port.
pub struct KubernetesLeaseElector {
    client: Client,
    k8s_api_url: String,
    namespace: String,
    lease_name: String,
    identity: String,
    token: Option<String>,
    is_leader: AtomicBool,
}

impl KubernetesLeaseElector {
    pub fn new(
        lease_name: impl Into<String>,
        namespace: impl Into<String>,
        identity: impl Into<String>,
        k8s_api_url: Option<String>,
        token: Option<String>,
    ) -> Self {
        Self::new_with_timeout(
            lease_name,
            namespace,
            identity,
            k8s_api_url,
            token,
            DEFAULT_REQUEST_TIMEOUT,
        )
    }

    pub fn new_with_timeout(
        lease_name: impl Into<String>,
        namespace: impl Into<String>,
        identity: impl Into<String>,
        k8s_api_url: Option<String>,
        token: Option<String>,
        request_timeout: Duration,
    ) -> Self {
        let host =
            std::env::var("KUBERNETES_SERVICE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("KUBERNETES_SERVICE_PORT").unwrap_or_else(|_| "443".to_string());
        let default_url = format!("https://{host}:{port}");
        let token = token
            .or_else(|| std::env::var("KUBERNETES_BEARER_TOKEN").ok())
            .or_else(|| {
                std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/token").ok()
            });
        let client = Client::builder()
            .timeout(request_timeout)
            .build()
            .expect("valid Kubernetes HTTP client configuration");

        Self {
            client,
            k8s_api_url: k8s_api_url
                .unwrap_or(default_url)
                .trim_end_matches('/')
                .to_string(),
            namespace: namespace.into(),
            lease_name: lease_name.into(),
            identity: identity.into(),
            token,
            is_leader: AtomicBool::new(false),
        }
    }

    fn lease_url(&self) -> String {
        format!(
            "{}/apis/coordination.k8s.io/v1/namespaces/{}/leases/{}",
            self.k8s_api_url, self.namespace, self.lease_name
        )
    }

    fn leases_url(&self) -> String {
        format!(
            "{}/apis/coordination.k8s.io/v1/namespaces/{}/leases",
            self.k8s_api_url, self.namespace
        )
    }

    fn authenticate(&self, request: RequestBuilder) -> RequestBuilder {
        if let Some(token) = self.token.as_deref() {
            return request.bearer_auth(token);
        }
        request
    }

    fn fail_closed(&self, reason: &str) -> bool {
        warn!(
            reason,
            "Kubernetes Lease operation failed; remaining standby"
        );
        self.is_leader.store(false, Ordering::SeqCst);
        false
    }

    async fn get_lease(&self) -> Option<LeaseObject> {
        let response = match self
            .authenticate(self.client.get(self.lease_url()))
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.fail_closed(&format!("request error: {error}"));
                return None;
            }
        };
        if response.status() == StatusCode::NOT_FOUND {
            return self.create_lease().await;
        }
        if !response.status().is_success() {
            self.fail_closed(&format!("HTTP status {}", response.status()));
            return None;
        }
        match response.json::<LeaseObject>().await {
            Ok(lease) => Some(lease),
            Err(error) => {
                self.fail_closed(&format!("invalid Lease response: {error}"));
                None
            }
        }
    }
}

#[async_trait]
impl LeaderElectionPort for KubernetesLeaseElector {
    async fn try_acquire_or_renew(&self) -> LogrixResult<bool> {
        let Some(current) = self.get_lease().await else {
            return Ok(false);
        };

        if let Some(holder) = current.spec.holder_identity.as_deref() {
            if holder != self.identity {
                match self.lease_expired(&current) {
                    Some(true) => {}
                    Some(false) => return Ok(false),
                    None => {
                        self.fail_closed("unable to determine current Lease expiry");
                        return Ok(false);
                    }
                }
            }
        }

        let Some(updated) = self.update_lease(current).await else {
            return Ok(false);
        };
        let acquired = updated.spec.holder_identity.as_deref() == Some(self.identity.as_str());
        self.is_leader.store(acquired, Ordering::SeqCst);
        Ok(acquired)
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
