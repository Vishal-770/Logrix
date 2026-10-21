use crate::config::RabbitMQConfig;
use crate::topology::QueueTopology;
use lapin::{Channel, Connection, ConnectionProperties};
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::sync::Arc;
use tracing::info;

/// RabbitMQ AMQP implementation of `QueuePort` with bounded prefetch and manual ACKs.
#[derive(Clone, Debug)]
pub struct RabbitMQQueue {
    _conn: Arc<Connection>,
    channel: Arc<Channel>,
    topology: QueueTopology,
}

impl RabbitMQQueue {
    /// Connect to RabbitMQ using default configuration.
    pub async fn connect(amqp_url: &str, prefetch_count: u16) -> LogrixResult<Self> {
        let config = RabbitMQConfig::new(amqp_url).prefetch_count(prefetch_count);
        Self::connect_with_config(&config).await
    }

    /// Connect to RabbitMQ using user-customized configuration.
    pub async fn connect_with_config(config: &RabbitMQConfig) -> LogrixResult<Self> {
        let setup = async {
            let conn = Connection::connect(&config.amqp_url, ConnectionProperties::default())
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::Queue,
                        format!(
                            "Failed to connect to RabbitMQ at {}: {e}",
                            config.redacted_url()
                        ),
                    )
                })?;

            let channel = conn.create_channel().await.map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to create AMQP channel: {e}"),
                )
            })?;

            let topology = QueueTopology::from_config(config);
            topology.declare(&channel).await?;

            channel
                .basic_qos(
                    config.prefetch_count,
                    lapin::options::BasicQosOptions::default(),
                )
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::Queue,
                        format!("Failed to set QoS prefetch: {e}"),
                    )
                })?;

            Ok((conn, channel, topology))
        };

        let (conn, channel, topology) = tokio::time::timeout(config.connection_timeout, setup)
            .await
            .map_err(|_| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!(
                        "RabbitMQ connection and topology setup timed out after {:?}",
                        config.connection_timeout
                    ),
                )
            })??;

        info!(
            prefetch = config.prefetch_count,
            exchange = %config.exchange_name,
            "RabbitMQ topology initialized successfully"
        );

        Ok(Self {
            _conn: Arc::new(conn),
            channel: Arc::new(channel),
            topology,
        })
    }

    /// Access internal channel.
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// Access internal topology.
    pub fn topology(&self) -> &QueueTopology {
        &self.topology
    }
}
