//! Production-grade Webhook Engine for Logrix.
//!
//! Provides HMAC-SHA256 signature calculation, standard headers, database persistence,
//! and standalone consumer service for dedicated Kubernetes webhook pods.

pub mod client;
pub mod dispatcher;
pub mod signature;
pub mod store;

pub use client::{DeliveryResult, WebhookHttpClient};
pub use dispatcher::WebhookDispatcherService;
pub use signature::{generate_secret, sign_payload, verify_signature};
pub use store::WebhookStore;
