//! AWS SQS queue adapter implementing `QueuePort`.
//!
//! Provides cloud-native queueing on AWS (EKS / ECS / EC2), IAM authentication,
//! and LocalStack endpoint override for local testing.

pub mod queue;

pub use queue::SqsQueue;
