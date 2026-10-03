locals {
  suffix = var.enable_fifo ? ".fifo" : ""
}

resource "aws_sqs_queue" "dlq" {
  name                      = "${var.project_name}-${var.environment}-dlq${local.suffix}"
  fifo_queue                = var.enable_fifo
  message_retention_seconds = var.dlq_retention_seconds
  sqs_managed_sse_enabled   = true

  tags = {
    Name        = "${var.project_name}-${var.environment}-dlq"
    Environment = var.environment
  }
}

resource "aws_sqs_queue" "live" {
  name                       = "${var.project_name}-${var.environment}-live${local.suffix}"
  fifo_queue                 = var.enable_fifo
  visibility_timeout_seconds = var.visibility_timeout_seconds
  message_retention_seconds  = var.message_retention_seconds
  sqs_managed_sse_enabled    = true

  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.dlq.arn
    maxReceiveCount     = var.max_receive_count
  })

  tags = {
    Name        = "${var.project_name}-${var.environment}-live"
    Environment = var.environment
  }
}

resource "aws_sqs_queue" "backfill" {
  name                       = "${var.project_name}-${var.environment}-backfill${local.suffix}"
  fifo_queue                 = var.enable_fifo
  visibility_timeout_seconds = var.visibility_timeout_seconds
  message_retention_seconds  = var.message_retention_seconds
  sqs_managed_sse_enabled    = true

  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.dlq.arn
    maxReceiveCount     = var.max_receive_count
  })

  tags = {
    Name        = "${var.project_name}-${var.environment}-backfill"
    Environment = var.environment
  }
}

resource "aws_sqs_queue" "webhook" {
  name                       = "${var.project_name}-${var.environment}-webhook${local.suffix}"
  fifo_queue                 = var.enable_fifo
  visibility_timeout_seconds = var.visibility_timeout_seconds
  message_retention_seconds  = var.message_retention_seconds
  sqs_managed_sse_enabled    = true

  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.dlq.arn
    maxReceiveCount     = var.max_receive_count
  })

  tags = {
    Name        = "${var.project_name}-${var.environment}-webhook"
    Environment = var.environment
  }
}
