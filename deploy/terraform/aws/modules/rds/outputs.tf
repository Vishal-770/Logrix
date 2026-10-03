output "cluster_endpoint" {
  description = "The primary writer endpoint of the Aurora PostgreSQL cluster"
  value       = aws_rds_cluster.aurora.endpoint
}

output "reader_endpoint" {
  description = "The reader endpoint of the Aurora PostgreSQL cluster"
  value       = aws_rds_cluster.aurora.reader_endpoint
}

output "database_name" {
  description = "The database name"
  value       = aws_rds_cluster.aurora.database_name
}

output "secret_arn" {
  description = "ARN of the Secrets Manager secret containing database credentials"
  value       = aws_secretsmanager_secret.db_credentials.arn
}

output "security_group_id" {
  description = "Security group ID of the Aurora PostgreSQL cluster"
  value       = aws_security_group.rds.id
}
