use crate::scaffold::ScaffoldConfig;
use std::fs;

pub fn write_terraform_files(cfg: &ScaffoldConfig) -> std::io::Result<()> {
    let tf_dir = cfg.target_dir.join("infra").join("terraform");
    fs::create_dir_all(&tf_dir)?;

    fs::write(tf_dir.join("main.tf"), main_tf_template())?;
    fs::write(
        tf_dir.join("variables.tf"),
        variables_tf_template(&cfg.name),
    )?;
    fs::write(tf_dir.join("outputs.tf"), outputs_tf_template())?;
    fs::write(
        tf_dir.join("terraform.tfvars.example"),
        tfvars_template(&cfg.name),
    )?;

    Ok(())
}

fn main_tf_template() -> &'static str {
    r#"terraform {
  required_version = ">= 1.5.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = var.project_name
      Environment = var.environment
      ManagedBy   = "Terraform"
    }
  }
}

resource "aws_s3_bucket" "cold_storage" {
  bucket        = "${var.project_name}-${var.environment}-cold-blobs"
  force_destroy = false
}

resource "aws_sqs_queue" "events_queue" {
  name                       = "${var.project_name}-${var.environment}-events"
  message_retention_seconds  = 86400
  visibility_timeout_seconds = 300
}

resource "aws_iam_role" "logrix_irsa" {
  name = "${var.project_name}-${var.environment}-processor-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRoleWithWebIdentity"
      Effect = "Allow"
      Principal = {
        Federated = "arn:aws:iam::${var.aws_account_id}:oidc-provider/${var.oidc_provider}"
      }
    }]
  })
}
"#
}

fn variables_tf_template(name: &str) -> String {
    format!(
        r#"variable "aws_region" {{
  type    = string
  default = "us-east-1"
}}

variable "aws_account_id" {{
  type    = string
  default = "123456789012"
}}

variable "oidc_provider" {{
  type    = string
  default = "oidc.eks.us-east-1.amazonaws.com/id/EXAMPLE"
}}

variable "project_name" {{
  type    = string
  default = "{name}"
}}

variable "environment" {{
  type    = string
  default = "prod"
}}
"#,
        name = name
    )
}

fn outputs_tf_template() -> &'static str {
    r#"output "sqs_queue_url" {
  description = "SQS Queue URL for Logrix Ingester and Processor"
  value       = aws_sqs_queue.events_queue.url
}

output "s3_bucket_name" {
  description = "S3 Bucket Name for cold storage blobs"
  value       = aws_s3_bucket.cold_storage.bucket
}

output "iam_role_arn" {
  description = "IAM Role ARN to attach to Helm serviceAccount.annotations"
  value       = aws_iam_role.logrix_irsa.arn
}
"#
}

fn tfvars_template(name: &str) -> String {
    format!(
        r#"aws_region     = "us-east-1"
aws_account_id = "123456789012"
oidc_provider  = "oidc.eks.us-east-1.amazonaws.com/id/EXAMPLE"
project_name   = "{name}"
environment    = "prod"
"#,
        name = name
    )
}
