//! Configuration parameters and builder for the Logrix GraphQL API server.

pub const DEFAULT_MAX_DEPTH: usize = 7;
pub const DEFAULT_MAX_COMPLEXITY: usize = 200;
pub const DEFAULT_QUERY_LIMIT: usize = 100;
pub const DEFAULT_MAX_QUERY_LIMIT: usize = 1000;
pub const DEFAULT_QUERY_TIMEOUT_SECS: u64 = 5;
pub const DEFAULT_SUBSCRIPTION_CHANNEL_CAPACITY: usize = 1024;

/// Configuration for the Logrix GraphQL API server and query guardrails.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub max_depth: usize,
    pub max_complexity: usize,
    pub default_limit: usize,
    pub max_limit: usize,
    pub query_timeout_secs: u64,
    pub chain_id: Option<u64>,
    pub allowed_origins: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_complexity: DEFAULT_MAX_COMPLEXITY,
            default_limit: DEFAULT_QUERY_LIMIT,
            max_limit: DEFAULT_MAX_QUERY_LIMIT,
            query_timeout_secs: DEFAULT_QUERY_TIMEOUT_SECS,
            chain_id: None,
            allowed_origins: Vec::new(),
        }
    }
}

impl ApiConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_max_complexity(mut self, complexity: usize) -> Self {
        self.max_complexity = complexity;
        self
    }

    pub fn with_default_limit(mut self, limit: usize) -> Self {
        self.default_limit = limit;
        self
    }

    pub fn with_max_limit(mut self, max: usize) -> Self {
        self.max_limit = max;
        self
    }

    pub fn with_query_timeout_secs(mut self, secs: u64) -> Self {
        self.query_timeout_secs = secs;
        self
    }

    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    pub fn with_allowed_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }
}
