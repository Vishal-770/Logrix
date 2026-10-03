use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::info;

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
    /// Create a new PostgresStore with production-tuned pool settings.
    pub async fn connect(database_url: &str, pipeline_id: impl Into<String>) -> LogrixResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(30)
            .min_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(600))
            .connect(database_url)
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
            pipeline_id: pipeline_id.into(),
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
}
