variable "project_name" {
  type        = string
  description = "Project name identifier"
}

variable "environment" {
  type        = string
  description = "Deployment environment"
}

variable "bucket_name" {
  type        = string
  description = "Optional explicit bucket name. If null, a deterministic name is generated"
  default     = null
}

variable "enable_versioning" {
  type        = bool
  description = "Whether to enable bucket versioning"
  default     = true
}

variable "standard_ia_transition_days" {
  type        = number
  description = "Days before transitioning raw block archives to Standard-IA"
  default     = 30
}

variable "glacier_ir_transition_days" {
  type        = number
  description = "Days before transitioning raw block archives to Glacier Instant Retrieval"
  default     = 90
}
