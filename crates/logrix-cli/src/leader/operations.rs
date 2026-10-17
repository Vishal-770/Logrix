use super::types::{LeaseMetadata, LeaseObject, LeaseSpec, DEFAULT_LEASE_DURATION_SECONDS};
use super::KubernetesLeaseElector;
use chrono::{DateTime, Duration as ChronoDuration, Utc};

impl KubernetesLeaseElector {
    pub(super) async fn create_lease(&self) -> Option<LeaseObject> {
        let now = Utc::now().to_rfc3339();
        let lease = LeaseObject::new(
            LeaseMetadata {
                name: self.lease_name.clone(),
                namespace: self.namespace.clone(),
                resource_version: None,
            },
            LeaseSpec {
                holder_identity: Some(self.identity.clone()),
                lease_duration_seconds: DEFAULT_LEASE_DURATION_SECONDS,
                acquire_time: Some(now.clone()),
                renew_time: Some(now),
            },
        );
        let response = match self
            .authenticate(self.client.post(self.leases_url()))
            .json(&lease)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.fail_closed(&format!("create request error: {error}"));
                return None;
            }
        };
        if !response.status().is_success() {
            self.fail_closed(&format!("create HTTP status {}", response.status()));
            return None;
        }
        self.parse_successful_lease(response).await
    }

    async fn parse_successful_lease(&self, response: reqwest::Response) -> Option<LeaseObject> {
        match response.json::<LeaseObject>().await {
            Ok(lease) => Some(lease),
            Err(error) => {
                self.fail_closed(&format!("invalid Lease response: {error}"));
                None
            }
        }
    }

    pub(super) fn lease_expired(&self, lease: &LeaseObject) -> Option<bool> {
        let timestamp = lease
            .spec
            .renew_time
            .as_deref()
            .or(lease.spec.acquire_time.as_deref())?;
        let renewed_at = DateTime::parse_from_rfc3339(timestamp).ok()?;
        let duration = ChronoDuration::seconds(i64::from(lease.spec.lease_duration_seconds));
        Some(Utc::now() >= renewed_at.with_timezone(&Utc) + duration)
    }

    pub(super) async fn update_lease(&self, current: LeaseObject) -> Option<LeaseObject> {
        let now = Utc::now().to_rfc3339();
        let is_renewal = current.spec.holder_identity.as_deref() == Some(self.identity.as_str());
        let duration = current
            .spec
            .lease_duration_seconds
            .max(DEFAULT_LEASE_DURATION_SECONDS);
        let acquire_time = if is_renewal {
            current.spec.acquire_time.clone()
        } else {
            Some(now.clone())
        };
        let lease = LeaseObject::new(
            current.metadata,
            LeaseSpec {
                holder_identity: Some(self.identity.clone()),
                lease_duration_seconds: duration,
                acquire_time,
                renew_time: Some(now),
            },
        );
        let response = match self
            .authenticate(self.client.put(self.lease_url()))
            .json(&lease)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.fail_closed(&format!("update request error: {error}"));
                return None;
            }
        };
        if !response.status().is_success() {
            self.fail_closed(&format!("update HTTP status {}", response.status()));
            return None;
        }
        self.parse_successful_lease(response).await
    }
}
