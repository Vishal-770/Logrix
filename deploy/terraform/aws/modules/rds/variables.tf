variable "project_name" {
  type        = string
  description = "Project name identifier"
}

variable "environment" {
  type        = string
  description = "Deployment environment"
}

variable "vpc_id" {
  type        = string
  description = "VPC ID where Aurora RDS is deployed"
}

variable "subnet_ids" {
  type        = list(string)
  description = "Private subnet IDs for DB Subnet Group"
}

variable "client_security_group_id" {
  type        = string
  description = "Security group ID of the EKS cluster/workers allowed to connect to Postgres"
}

variable "min_capacity" {
  type        = number
  description = "Minimum Aurora Serverless v2 capacity units (ACUs)"
  default     = 0.5
}

variable "max_capacity" {
  type        = number
  description = "Maximum Aurora Serverless v2 capacity units (ACUs)"
  default     = 16.0
}

variable "database_name" {
  type        = string
  description = "Initial database name"
  default     = "logrix"
}

variable "master_username" {
  type        = string
  description = "Master database username"
  default     = "logrix_admin"
}
