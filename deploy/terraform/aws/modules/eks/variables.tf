variable "cluster_name" {
  type        = string
  description = "EKS cluster name"
}

variable "cluster_version" {
  type        = string
  description = "Kubernetes control plane version"
  default     = "1.30"
}

variable "vpc_id" {
  type        = string
  description = "VPC ID where EKS is deployed"
}

variable "subnet_ids" {
  type        = list(string)
  description = "Private subnet IDs for worker nodes and control plane"
}

variable "instance_types" {
  type        = list(string)
  description = "EC2 instance types for managed node group"
  default     = ["m6i.xlarge"]
}

variable "desired_size" {
  type        = number
  description = "Desired number of worker nodes"
  default     = 3
}

variable "min_size" {
  type        = number
  description = "Minimum number of worker nodes"
  default     = 2
}

variable "max_size" {
  type        = number
  description = "Maximum number of worker nodes"
  default     = 20
}

variable "environment" {
  type        = string
  description = "Deployment environment"
}
