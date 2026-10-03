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

locals {
  cluster_name = "${var.project_name}-${var.environment}-eks"
}

module "vpc" {
  source = "./modules/vpc"

  project_name = var.project_name
  environment  = var.environment
  cidr_block   = var.vpc_cidr
  cluster_name = local.cluster_name
}

module "eks" {
  source = "./modules/eks"

  cluster_name    = local.cluster_name
  cluster_version = var.eks_cluster_version
  vpc_id          = module.vpc.vpc_id
  subnet_ids      = module.vpc.private_subnet_ids
  instance_types  = var.eks_node_instance_types
  desired_size    = var.eks_node_desired_size
  min_size        = var.eks_node_min_size
  max_size        = var.eks_node_max_size
  environment     = var.environment
}

module "rds" {
  source = "./modules/rds"

  project_name             = var.project_name
  environment              = var.environment
  vpc_id                   = module.vpc.vpc_id
  subnet_ids               = module.vpc.private_subnet_ids
  client_security_group_id = module.eks.cluster_security_group_id
  min_capacity             = var.rds_min_acu
  max_capacity             = var.rds_max_acu
  database_name            = var.rds_database_name
  master_username          = var.rds_master_username
}

module "sqs" {
  source = "./modules/sqs"

  project_name = var.project_name
  environment  = var.environment
  enable_fifo  = var.sqs_enable_fifo
}

module "s3" {
  source = "./modules/s3"

  project_name = var.project_name
  environment  = var.environment
  bucket_name  = var.s3_bucket_name
}

module "iam" {
  source = "./modules/iam"

  project_name      = var.project_name
  environment       = var.environment
  oidc_provider_arn = module.eks.oidc_provider_arn
  oidc_provider_url = module.eks.oidc_provider_url
  k8s_namespace     = var.k8s_namespace
  s3_bucket_arn     = module.s3.bucket_arn
  sqs_queue_arns = [
    module.sqs.live_queue_arn,
    module.sqs.backfill_queue_arn,
    module.sqs.webhook_queue_arn,
    module.sqs.dlq_arn,
  ]
  db_secret_arn = module.rds.secret_arn
}
