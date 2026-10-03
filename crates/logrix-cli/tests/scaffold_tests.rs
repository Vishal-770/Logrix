use logrix_cli::scaffold::{scaffold_project, InfraProfile, LogicStrategy, ScaffoldConfig};
use std::fs;
use uuid::Uuid;

fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
    let _ = fs::create_dir_all(&dir);
    dir
}

#[test]
fn test_scaffold_local_docker_profile() {
    let project_dir = make_temp_dir("docker-indexer");

    let cfg = ScaffoldConfig {
        name: "my-docker-indexer".into(),
        profile: InfraProfile::LocalDocker,
        chain_id: 42161,
        rpc_url: "https://arb1.arbitrum.io/rpc".into(),
        contract_address: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".into(),
        start_block: Some(220000000),
        logic: LogicStrategy::Declarative,
        enable_webhooks: true,
        enable_s3: true,
        target_dir: project_dir.clone(),
    };

    scaffold_project(&cfg).expect("scaffold failed");

    assert!(project_dir.join("manifest.yaml").exists());
    assert!(project_dir.join("schema.graphql").exists());
    assert!(project_dir.join("docker-compose.yml").exists());
    assert!(project_dir.join(".env").exists());
    assert!(project_dir.join("README.md").exists());

    let env_content = fs::read_to_string(project_dir.join(".env")).unwrap();
    assert!(env_content.contains("QUEUE_DRIVER=rabbitmq"));
    assert!(env_content.contains("CHAIN_ID=42161"));

    let manifest_content = fs::read_to_string(project_dir.join("manifest.yaml")).unwrap();
    assert!(manifest_content.contains("0xaf88d065e77c8cC2239327C5EDb3A432268e5831"));
}

#[test]
fn test_scaffold_aws_profile_and_wasm() {
    let project_dir = make_temp_dir("aws-indexer");

    let cfg = ScaffoldConfig {
        name: "my-aws-indexer".into(),
        profile: InfraProfile::AwsTerraform,
        chain_id: 1,
        rpc_url: "https://eth.llamarpc.com".into(),
        contract_address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".into(),
        start_block: None,
        logic: LogicStrategy::TypeScriptWasm,
        enable_webhooks: true,
        enable_s3: true,
        target_dir: project_dir.clone(),
    };

    scaffold_project(&cfg).expect("scaffold failed");

    assert!(project_dir.join("handlers/mapping.ts").exists());
    assert!(project_dir.join("handlers/types.ts").exists());
    assert!(project_dir.join("package.json").exists());
    assert!(project_dir.join("tsconfig.json").exists());
    let env_content = fs::read_to_string(project_dir.join(".env")).unwrap();
    assert!(env_content.contains("QUEUE_DRIVER=sqs"));
    assert!(env_content.contains("CHAIN_ID=1"));
}

#[test]
fn test_scaffold_custom_byo_profile() {
    let project_dir = make_temp_dir("byo-indexer");

    let cfg = ScaffoldConfig {
        name: "my-byo-indexer".into(),
        profile: InfraProfile::CustomByo,
        chain_id: 8453,
        rpc_url: "https://mainnet.base.org".into(),
        contract_address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".into(),
        start_block: Some(1000000),
        logic: LogicStrategy::Declarative,
        enable_webhooks: false,
        enable_s3: false,
        target_dir: project_dir.clone(),
    };

    scaffold_project(&cfg).expect("scaffold failed");

    assert!(!project_dir.join("docker-compose.yml").exists());
    let env_content = fs::read_to_string(project_dir.join(".env")).unwrap();
    assert!(env_content.contains("QUEUE_DRIVER=memory"));
}
