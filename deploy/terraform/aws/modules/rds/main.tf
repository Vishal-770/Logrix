resource "random_password" "master_password" {
  length           = 32
  special          = true
  override_special = "!#$%&*()-_=+[]{}<>:?"
}

resource "aws_db_subnet_group" "this" {
  name       = "${var.project_name}-${var.environment}-rds-subnet-grp"
  subnet_ids = var.subnet_ids

  tags = {
    Name        = "${var.project_name}-${var.environment}-rds-subnet-grp"
    Environment = var.environment
  }
}

resource "aws_security_group" "rds" {
  name        = "${var.project_name}-${var.environment}-rds-sg"
  description = "Access to Aurora PostgreSQL from Logrix EKS workers"
  vpc_id      = var.vpc_id

  ingress {
    description     = "PostgreSQL access from EKS workers"
    from_port       = 5432
    to_port         = 5432
    protocol        = "tcp"
    security_groups = [var.client_security_group_id]
  }

  egress {
    from_port        = 0
    to_port          = 0
    protocol         = "-1"
    cidr_blocks      = ["0.0.0.0/0"]
    ipv6_cidr_blocks = ["::/0"]
  }

  tags = {
    Name        = "${var.project_name}-${var.environment}-rds-sg"
    Environment = var.environment
  }
}

resource "aws_rds_cluster_parameter_group" "this" {
  name        = "${var.project_name}-${var.environment}-pg16-params"
  family      = "aurora-postgresql16"
  description = "Tuned PostgreSQL 16 parameters for Logrix high-throughput blockchain indexing"

  parameter {
    name  = "rds.force_ssl"
    value = "1"
  }

  parameter {
    name  = "work_mem"
    value = "65536"
  }

  parameter {
    name  = "maintenance_work_mem"
    value = "524288"
  }

  parameter {
    name  = "checkpoint_completion_target"
    value = "0.9"
  }
}

resource "aws_rds_cluster" "aurora" {
  cluster_identifier              = "${var.project_name}-${var.environment}-aurora"
  engine                          = "aurora-postgresql"
  engine_mode                     = "provisioned"
  engine_version                  = "16.2"
  database_name                   = var.database_name
  master_username                 = var.master_username
  master_password                 = random_password.master_password.result
  db_subnet_group_name            = aws_db_subnet_group.this.name
  vpc_security_group_ids          = [aws_security_group.rds.id]
  db_cluster_parameter_group_name = aws_rds_cluster_parameter_group.this.name
  storage_encrypted               = true
  deletion_protection             = false
  skip_final_snapshot             = true

  serverlessv2_scaling_configuration {
    min_capacity = var.min_capacity
    max_capacity = var.max_capacity
  }

  tags = {
    Name        = "${var.project_name}-${var.environment}-aurora"
    Environment = var.environment
  }
}

resource "aws_rds_cluster_instance" "writer" {
  cluster_identifier = aws_rds_cluster.aurora.id
  instance_class     = "db.serverless"
  engine             = aws_rds_cluster.aurora.engine
  engine_version     = aws_rds_cluster.aurora.engine_version
  identifier         = "${var.project_name}-${var.environment}-writer"

  tags = {
    Name        = "${var.project_name}-${var.environment}-writer"
    Environment = var.environment
  }
}

resource "aws_secretsmanager_secret" "db_credentials" {
  name_prefix             = "${var.project_name}-${var.environment}-rds-creds-"
  recovery_window_in_days = 0

  tags = {
    Environment = var.environment
  }
}

resource "aws_secretsmanager_secret_version" "db_credentials" {
  secret_id = aws_secretsmanager_secret.db_credentials.id
  secret_string = jsonencode({
    host     = aws_rds_cluster.aurora.endpoint
    port     = 5432
    database = var.database_name
    username = var.master_username
    password = random_password.master_password.result
    url      = "postgres://${var.master_username}:${urlencode(random_password.master_password.result)}@${aws_rds_cluster.aurora.endpoint}:5432/${var.database_name}?sslmode=require"
  })
}
