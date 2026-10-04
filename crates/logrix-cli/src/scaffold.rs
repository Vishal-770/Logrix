use crate::scaffold_templates::{
    docker_compose_template, package_json_template, readme_startup_steps, schema_graphql_template,
    sdk_types_template, ts_handler_template, tsconfig_template,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfraProfile {
    LocalDocker,
    AwsTerraform,
    CustomByo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicStrategy {
    Declarative,
    TypeScriptWasm,
}

#[derive(Debug, Clone)]
pub struct ScaffoldConfig {
    pub name: String,
    pub profile: InfraProfile,
    pub chain_id: u64,
    pub rpc_url: String,
    pub contract_address: String,
    pub start_block: Option<u64>,
    pub logic: LogicStrategy,
    pub enable_webhooks: bool,
    pub enable_s3: bool,
    pub target_dir: PathBuf,
}

pub fn scaffold_project(cfg: &ScaffoldConfig) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(&cfg.target_dir)?;
    write_manifest(cfg)?;
    write_schema(&cfg.target_dir)?;
    write_env(cfg)?;
    write_readme(cfg)?;

    if cfg.profile == InfraProfile::LocalDocker {
        let dc = cfg.target_dir.join("docker-compose.yml");
        fs::write(dc, docker_compose_template())?;
    }

    fs::write(
        cfg.target_dir.join("values-local.yaml"),
        crate::scaffold_helm::values_local_template(cfg),
    )?;

    if cfg.profile == InfraProfile::AwsTerraform {
        fs::write(
            cfg.target_dir.join("values-aws.yaml"),
            crate::scaffold_helm::values_aws_template(cfg),
        )?;
        crate::scaffold_terraform::write_terraform_files(cfg)?;
    }

    if cfg.logic == LogicStrategy::TypeScriptWasm {
        let handlers_dir = cfg.target_dir.join("handlers");
        fs::create_dir_all(&handlers_dir)?;
        fs::write(handlers_dir.join("mapping.ts"), ts_handler_template())?;
        fs::write(handlers_dir.join("types.ts"), sdk_types_template())?;
        fs::write(
            cfg.target_dir.join("package.json"),
            package_json_template(&cfg.name),
        )?;
        fs::write(cfg.target_dir.join("tsconfig.json"), tsconfig_template())?;
    }

    Ok(())
}

fn write_manifest(cfg: &ScaffoldConfig) -> std::io::Result<()> {
    let start_blk = cfg.start_block.unwrap_or(0);
    let handler_field = if cfg.logic == LogicStrategy::TypeScriptWasm {
        "        wasm_handler: \"./handlers/mapping.wasm\"\n"
    } else {
        ""
    };

    let content = format!(
        r#"version: "0.1.0"
network:
  chain_id: {chain_id}
  rpc_url: "{rpc_url}"

contracts:
  - name: "{name}Contract"
    address: "{addr}"
    start_block: {start_blk}
{handler_field}    events:
      - name: "Transfer"
        signature: "Transfer(address,address,uint256)"
"#,
        chain_id = cfg.chain_id,
        rpc_url = cfg.rpc_url,
        name = cfg.name,
        addr = cfg.contract_address,
        start_blk = start_blk,
        handler_field = handler_field
    );
    fs::write(cfg.target_dir.join("manifest.yaml"), content)
}

fn write_schema(dir: &Path) -> std::io::Result<()> {
    fs::write(dir.join("schema.graphql"), schema_graphql_template())
}

fn write_env(cfg: &ScaffoldConfig) -> std::io::Result<()> {
    let (db_url, queue_driver, rabbit_url, s3_bucket) = match cfg.profile {
        InfraProfile::LocalDocker => (
            "postgres://logrix:logrixpassword@localhost:5432/logrix",
            "rabbitmq",
            "amqp://logrix:logrixpassword@localhost:5672/%2f",
            if cfg.enable_s3 { "logrix-cold-blobs" } else { "" },
        ),
        InfraProfile::AwsTerraform => (
            "postgres://admin:YOUR_AURORA_PASSWORD@aurora-cluster.rds.amazonaws.com:5432/logrix?sslmode=require",
            "sqs",
            "",
            if cfg.enable_s3 { "my-logrix-aws-cold-blobs" } else { "" },
        ),
        InfraProfile::CustomByo => (
            "postgres://user:pass@your-database-host:5432/logrix",
            "memory",
            "",
            "",
        ),
    };

    let s3_endpoint = if cfg.profile == InfraProfile::LocalDocker && cfg.enable_s3 {
        "http://localhost:9000"
    } else {
        ""
    };

    let content = format!(
        r#"CHAIN_ID={chain_id}
RPC_URL={rpc_url}
CONTRACT_ADDRESS={addr}
DATABASE_URL={db_url}
QUEUE_DRIVER={queue_driver}
RABBITMQ_URL={rabbit_url}
ENABLE_WEBHOOKS={enable_wh}
WEBHOOK_SECRET=logrix_sec_{name}_dev
S3_BUCKET={s3_bucket}
S3_ENDPOINT={s3_endpoint}
"#,
        chain_id = cfg.chain_id,
        rpc_url = cfg.rpc_url,
        addr = cfg.contract_address,
        db_url = db_url,
        queue_driver = queue_driver,
        rabbit_url = rabbit_url,
        enable_wh = cfg.enable_webhooks,
        name = cfg.name,
        s3_bucket = s3_bucket,
        s3_endpoint = s3_endpoint
    );

    fs::write(cfg.target_dir.join(".env"), content)
}

fn write_readme(cfg: &ScaffoldConfig) -> std::io::Result<()> {
    let is_docker = cfg.profile == InfraProfile::LocalDocker;
    let is_aws = cfg.profile == InfraProfile::AwsTerraform;
    let startup_steps = readme_startup_steps(is_docker, is_aws);

    let content = format!(
        "# {name}\n\nBlockchain indexer scaffolded by Logrix.\n\n## Getting Started\n\n{startup_steps}\n\n### Access GraphQL Explorer\nOpen [http://localhost:4000/](http://localhost:4000/) in your browser.\n",
        name = cfg.name,
        startup_steps = startup_steps
    );

    fs::write(cfg.target_dir.join("README.md"), content)
}
