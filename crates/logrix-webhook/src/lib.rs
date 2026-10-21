//! Production-grade Webhook Engine for Logrix.
//!
//! Provides HMAC-SHA256 signature calculation, standard headers, database persistence,
//! and standalone consumer service for dedicated Kubernetes webhook pods.

pub mod client;
pub mod config;
pub mod deliveries;
pub mod dispatcher;
pub mod signature;
pub mod store;
pub mod target;

pub use client::{DeliveryResult, WebhookHttpClient};
pub use config::{
    WebhookConfig, DEFAULT_MAX_CONCURRENT_DISPATCHES, DEFAULT_MAX_RETRIES,
    DEFAULT_POOL_IDLE_TIMEOUT, DEFAULT_POOL_MAX_IDLE_PER_HOST, DEFAULT_RETRY_INITIAL_DELAY,
    DEFAULT_RETRY_MAX_DELAY, DEFAULT_TIMEOUT, DEFAULT_WORKER_ERROR_BACKOFF,
    DEFAULT_WORKER_IDLE_SLEEP,
};
pub use deliveries::DeliveryOps;
pub use dispatcher::WebhookDispatcherService;
pub use signature::{generate_secret, sanitize_webhook_url, sign_payload, verify_signature};
pub use store::WebhookStore;
pub use target::DispatchTargetOps;
