use std::time::Duration;

/// Default HTTP request timeout (5 seconds).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default maximum retry attempts for dispatch (1 attempt).
pub const DEFAULT_MAX_RETRIES: u32 = 1;

/// Default idle sleep when webhook queue is empty (100 ms).
pub const DEFAULT_WORKER_IDLE_SLEEP: Duration = Duration::from_millis(100);

/// Default backoff delay when an error occurs during queue processing (500 ms).
pub const DEFAULT_WORKER_ERROR_BACKOFF: Duration = Duration::from_millis(500);

/// Default HTTP connection pool idle timeout (90 seconds).
pub const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Default HTTP connection pool max idle connections per host (10).
pub const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 10;

/// Default maximum concurrent active webhook dispatches (50).
pub const DEFAULT_MAX_CONCURRENT_DISPATCHES: usize = 50;

/// Default initial retry delay for exponential backoff (200 ms).
pub const DEFAULT_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(200);

/// Default maximum retry delay for exponential backoff (4 seconds).
pub const DEFAULT_RETRY_MAX_DELAY: Duration = Duration::from_secs(4);

/// Configuration for Webhook HTTP dispatch and background polling service.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub worker_idle_sleep: Duration,
    pub worker_error_backoff: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub max_concurrent_dispatches: usize,
    pub retry_initial_delay: Duration,
    pub retry_max_delay: Duration,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            worker_idle_sleep: DEFAULT_WORKER_IDLE_SLEEP,
            worker_error_backoff: DEFAULT_WORKER_ERROR_BACKOFF,
            pool_idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
            pool_max_idle_per_host: DEFAULT_POOL_MAX_IDLE_PER_HOST,
            max_concurrent_dispatches: DEFAULT_MAX_CONCURRENT_DISPATCHES,
            retry_initial_delay: DEFAULT_RETRY_INITIAL_DELAY,
            retry_max_delay: DEFAULT_RETRY_MAX_DELAY,
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
            ..Default::default()
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_worker_idle_sleep(mut self, sleep: Duration) -> Self {
        self.worker_idle_sleep = sleep;
        self
    }

    pub fn with_worker_error_backoff(mut self, backoff: Duration) -> Self {
        self.worker_error_backoff = backoff;
        self
    }

    pub fn with_max_concurrent_dispatches(mut self, concurrency: usize) -> Self {
        self.max_concurrent_dispatches = concurrency;
        self
    }
}
