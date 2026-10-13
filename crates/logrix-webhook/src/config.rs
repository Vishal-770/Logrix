use std::time::Duration;

/// Configuration for Webhook HTTP dispatch and background polling service.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub worker_idle_sleep: Duration,
    pub worker_error_backoff: Duration,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_retries: 1,
            worker_idle_sleep: Duration::from_millis(100),
            worker_error_backoff: Duration::from_millis(500),
        }
    }
}

impl WebhookConfig {
    pub fn new(
        timeout: Duration,
        max_retries: u32,
        worker_idle_sleep: Duration,
        worker_error_backoff: Duration,
    ) -> Self {
        Self {
            timeout,
            max_retries,
            worker_idle_sleep,
            worker_error_backoff,
        }
    }
}
