output "live_queue_url" {
  description = "URL of the Live block ingestion SQS queue"
  value       = aws_sqs_queue.live.url
}

output "live_queue_arn" {
  description = "ARN of the Live block ingestion SQS queue"
  value       = aws_sqs_queue.live.arn
}

output "backfill_queue_url" {
  description = "URL of the Backfill historical range SQS queue"
  value       = aws_sqs_queue.backfill.url
}

output "backfill_queue_arn" {
  description = "ARN of the Backfill historical range SQS queue"
  value       = aws_sqs_queue.backfill.arn
}

output "webhook_queue_url" {
  description = "URL of the Webhook dispatch SQS queue"
  value       = aws_sqs_queue.webhook.url
}

output "webhook_queue_arn" {
  description = "ARN of the Webhook dispatch SQS queue"
  value       = aws_sqs_queue.webhook.arn
}

output "dlq_url" {
  description = "URL of the Dead-Letter SQS queue"
  value       = aws_sqs_queue.dlq.url
}

output "dlq_arn" {
  description = "ARN of the Dead-Letter SQS queue"
  value       = aws_sqs_queue.dlq.arn
}
