variable "project_name" {
  type        = string
  description = "Project name identifier"
}

variable "environment" {
  type        = string
  description = "Deployment environment"
}

variable "enable_fifo" {
  type        = bool
  description = "Whether to create FIFO queues instead of Standard queues"
  default     = false
}

variable "message_retention_seconds" {
  type        = number
  description = "SQS message retention period in seconds"
  default     = 345600 # 4 days
}

variable "dlq_retention_seconds" {
  type        = number
  description = "Dead-letter queue message retention period in seconds"
  default     = 1209600 # 14 days
}

variable "visibility_timeout_seconds" {
  type        = number
  description = "SQS message visibility timeout in seconds"
  default     = 300 # 5 minutes
}

variable "max_receive_count" {
  type        = number
  description = "Maximum attempts before redriving poisoned messages to the DLQ"
  default     = 5
}
