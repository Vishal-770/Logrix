//! AWS SQS queue adapter implementing `QueuePort`.
//!
//! Provides cloud-native queueing on AWS (EKS / ECS / EC2), IAM authentication,
//! and LocalStack endpoint override for local testing.

pub mod batch;
pub mod builder;
pub mod client;
pub mod port;
pub mod queue;

pub use builder::SqsConfigBuilder;
pub use client::SqsClientConfig;
pub use queue::SqsQueue;
