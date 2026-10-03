output "vpc_id" {
  description = "The ID of the VPC"
  value       = module.vpc.vpc_id
}

output "eks_cluster_name" {
  description = "The name of the EKS cluster"
  value       = module.eks.cluster_name
}

output "eks_cluster_endpoint" {
  description = "The endpoint URL for the EKS Kubernetes API"
  value       = module.eks.cluster_endpoint
}

output "rds_cluster_endpoint" {
  description = "The primary writer endpoint of the Aurora PostgreSQL cluster"
  value       = module.rds.cluster_endpoint
}

output "rds_secret_arn" {
  description = "ARN of the Secrets Manager secret containing database credentials"
  value       = module.rds.secret_arn
}

output "s3_bucket_name" {
  description = "The name of the S3 block archive bucket"
  value       = module.s3.bucket_id
}

output "sqs_live_queue_url" {
  description = "URL of the Live block ingestion SQS queue"
  value       = module.sqs.live_queue_url
}

output "sqs_backfill_queue_url" {
  description = "URL of the Backfill historical range SQS queue"
  value       = module.sqs.backfill_queue_url
}

output "irsa_ingester_role_arn" {
  description = "ARN of the IAM role for logrix-ingester service account"
  value       = module.iam.ingester_role_arn
}

output "irsa_processor_role_arn" {
  description = "ARN of the IAM role for logrix-processor service account"
  value       = module.iam.processor_role_arn
}

output "irsa_api_role_arn" {
  description = "ARN of the IAM role for logrix-api service account"
  value       = module.iam.api_role_arn
}
