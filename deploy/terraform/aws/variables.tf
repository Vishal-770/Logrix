variable "aws_region" {
  description = "AWS region for all deployed resources"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Deployment environment name (e.g. staging, production)"
  type        = string
  default     = "production"
}

variable "project_name" {
  description = "Project name identifier used for resource tagging and naming"
  type        = string
  default     = "logrix"
}

variable "vpc_cidr" {
  description = "CIDR block for the Logrix VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "eks_cluster_version" {
  description = "Kubernetes control plane version for EKS"
  type        = string
  default     = "1.30"
}

variable "eks_node_instance_types" {
  description = "Instance types for EKS managed worker nodes"
  type        = list(string)
  default     = ["m6i.xlarge", "c6i.xlarge"]
}

variable "eks_node_desired_size" {
  description = "Desired number of worker nodes"
  type        = number
  default     = 3
}

variable "eks_node_min_size" {
  description = "Minimum number of worker nodes for autoscaling"
  type        = number
  default     = 2
}

variable "eks_node_max_size" {
  description = "Maximum number of worker nodes for autoscaling"
  type        = number
  default     = 20
}

variable "rds_min_acu" {
  description = "Minimum Aurora Serverless v2 capacity units (0.5 to 128)"
  type        = number
  default     = 0.5
}

variable "rds_max_acu" {
  description = "Maximum Aurora Serverless v2 capacity units (0.5 to 128)"
  type        = number
  default     = 16.0
}

variable "rds_database_name" {
  description = "Default database name for Logrix indexer"
  type        = string
  default     = "logrix"
}

variable "rds_master_username" {
  description = "Master username for Aurora PostgreSQL"
  type        = string
  default     = "logrix_admin"
}

variable "sqs_enable_fifo" {
  description = "Whether to use SQS FIFO queues instead of Standard queues"
  type        = bool
  default     = false
}

variable "s3_bucket_name" {
  description = "Optional explicit name for the S3 block archive bucket"
  type        = string
  default     = null
}

variable "k8s_namespace" {
  description = "Kubernetes namespace where Logrix pods and service accounts are deployed"
  type        = string
  default     = "logrix"
}
