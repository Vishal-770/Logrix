variable "project_name" {
  type        = string
  description = "Project name identifier"
}

variable "environment" {
  type        = string
  description = "Deployment environment"
}

variable "oidc_provider_arn" {
  type        = string
  description = "ARN of the EKS OIDC Provider"
}

variable "oidc_provider_url" {
  type        = string
  description = "URL of the EKS OIDC Provider without https://"
}

variable "k8s_namespace" {
  type        = string
  description = "Kubernetes namespace where service accounts exist"
  default     = "logrix"
}

variable "s3_bucket_arn" {
  type        = string
  description = "ARN of the S3 block archive bucket"
}

variable "sqs_queue_arns" {
  type        = list(string)
  description = "List of ARNs for the SQS queues"
}

variable "db_secret_arn" {
  type        = string
  description = "ARN of the Secrets Manager database credentials secret"
}
