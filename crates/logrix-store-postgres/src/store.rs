use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::info;

/// Configuration for PostgreSQL connection pool.
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub database_url: String,
    pub pipeline_id: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
}

impl PostgresConfig {
    pub fn new(database_url: impl Into<String>, pipeline_id: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            pipeline_id: pipeline_id.into(),
            max_connections: 30,
            min_connections: 5,
            acquire_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(600),
        }
    }

    #[must_use]
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    #[must_use]
    pub fn min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    #[must_use]
    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }
}

/// Production PostgreSQL implementation of `StorePort`.
///
/// Features high-throughput vectorized UNNEST batch inserts, atomic transaction
/// boundaries across events and checkpoints, and automatic reorg rollbacks.
#[derive(Debug, Clone)]
pub struct PostgresStore {
    pool: PgPool,
    pipeline_id: String,
}

impl PostgresStore {
    /// Create a new PostgresStore with default connection pool settings.
    pub async fn connect(database_url: &str, pipeline_id: impl Into<String>) -> LogrixResult<Self> {
        let config = PostgresConfig::new(database_url, pipeline_id);
        Self::connect_with_config(&config).await
    }

    /// Create a new PostgresStore with user-customized pool configuration.
    pub async fn connect_with_config(config: &PostgresConfig) -> LogrixResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.acquire_timeout)
            .idle_timeout(config.idle_timeout)
            .connect(&config.database_url)
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to connect to PostgreSQL: {e}"),
                )
            })?;

        info!("Connected to PostgreSQL connection pool successfully");
        Ok(Self {
            pool,
            pipeline_id: config.pipeline_id.clone(),
        })
    }

    /// Construct from an existing sqlx PgPool.
    pub fn from_pool(pool: PgPool, pipeline_id: impl Into<String>) -> Self {
        Self {
            pool,
            pipeline_id: pipeline_id.into(),
        }
    }

    /// Run embedded database migrations automatically.
    pub async fn migrate(&self) -> LogrixResult<()> {
        info!("Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Database,
                    format!("Database migration failed: {e}"),
                )
            })?;
        info!("Database migrations applied successfully");
        Ok(())
    }

    /// Return reference to internal connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Return reference to pipeline ID string.
    pub fn pipeline_id(&self) -> &str {
        &self.pipeline_id
    }

    /// Execute a lightweight liveness check query against the database pool.
    pub async fn ping(&self) -> LogrixResult<()> {
        sqlx::query("SELECT 1")
            .execute(self.pool())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Database health ping failed: {e}"),
                )
            })?;
        Ok(())
    }
}
