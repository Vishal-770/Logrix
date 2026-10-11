use std::time::Duration;

pub const DEFAULT_EXCHANGE: &str = "logrix.events";
pub const DEFAULT_PREFETCH: u16 = 100;

/// RabbitMQ connection and topology configuration.
#[derive(Debug, Clone)]
pub struct RabbitMQConfig {
    pub amqp_url: String,
    pub prefetch_count: u16,
    pub exchange_name: String,
    pub queue_prefix: Option<String>,
    pub connection_timeout: Duration,
}

impl RabbitMQConfig {
    pub fn new(amqp_url: impl Into<String>) -> Self {
        Self {
            amqp_url: amqp_url.into(),
            prefetch_count: DEFAULT_PREFETCH,
            exchange_name: DEFAULT_EXCHANGE.to_string(),
            queue_prefix: None,
            connection_timeout: Duration::from_secs(10),
        }
    }

    #[must_use]
    pub fn prefetch_count(mut self, prefetch: u16) -> Self {
        self.prefetch_count = prefetch;
        self
    }

    #[must_use]
    pub fn exchange_name(mut self, exchange: impl Into<String>) -> Self {
        self.exchange_name = exchange.into();
        self
    }

    #[must_use]
    pub fn queue_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.queue_prefix = Some(prefix.into());
        self
    }

    #[must_use]
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Return sanitized AMQP URL with redacted password for safe logging.
    #[must_use]
    pub fn redacted_url(&self) -> String {
        redact_amqp_url(&self.amqp_url)
    }
}

/// Redact credentials in AMQP URI (e.g. `amqp://user:password@host` -> `amqp://user:***@host`).
pub fn redact_amqp_url(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((user_pass, host)) = rest.split_once('@') {
            if let Some((user, _pass)) = user_pass.split_once(':') {
                return format!("{scheme}://{user}:***@{host}");
            }
        }
    }
    url.to_string()
}
