output "ingester_role_arn" {
  description = "ARN of the IAM role for logrix-ingester service account"
  value       = aws_iam_role.ingester.arn
}

output "processor_role_arn" {
  description = "ARN of the IAM role for logrix-processor service account"
  value       = aws_iam_role.processor.arn
}

output "api_role_arn" {
  description = "ARN of the IAM role for logrix-api service account"
  value       = aws_iam_role.api.arn
}
