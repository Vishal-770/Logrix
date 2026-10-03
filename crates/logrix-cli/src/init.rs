use crate::scaffold::{scaffold_project, InfraProfile, LogicStrategy, ScaffoldConfig};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

pub struct InitOptions {
    pub name: Option<String>,
    pub profile: Option<String>,
    pub network: Option<String>,
    pub contract_address: Option<String>,
    pub start_block: Option<u64>,
    pub logic: Option<String>,
    pub non_interactive: bool,
}

pub fn run_init(opts: InitOptions) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 Welcome to Logrix! Let's scaffold your new blockchain indexer.\n");

    let name = if let Some(n) = opts.name {
        n
    } else if opts.non_interactive {
        "my-indexer".to_string()
    } else {
        prompt_with_default("Project name", "my-indexer")
    };

    let profile = parse_profile(opts.profile.as_deref(), opts.non_interactive);
    let (chain_id, rpc_url) = parse_network(opts.network.as_deref(), opts.non_interactive);

    let contract_address = if let Some(c) = opts.contract_address {
        c
    } else if opts.non_interactive {
        "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d".to_string()
    } else {
        prompt_with_default(
            "Target Contract Address",
            "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d",
        )
    };

    let logic = parse_logic(opts.logic.as_deref(), opts.non_interactive);
    let target_dir = PathBuf::from(&name);

    let cfg = ScaffoldConfig {
        name: name.clone(),
        profile,
        chain_id,
        rpc_url,
        contract_address,
        start_block: opts.start_block,
        logic,
        enable_webhooks: true,
        enable_s3: true,
        target_dir,
    };

    scaffold_project(&cfg)?;

    println!("\n✨ Successfully created project '{name}'!");
    println!("👉 Next steps:\n");
    match profile {
        InfraProfile::LocalDocker => {
            println!("  cd {name}");
            println!("  docker compose up -d   # Start Postgres, RabbitMQ & MinIO");
            println!("  logrix all-in-one       # Run your indexer!");
        }
        InfraProfile::AwsTerraform => {
            println!("  cd {name}");
            println!("  cd infra/terraform && terraform apply   # Provision AWS Aurora, SQS, S3");
            println!(
                "  helm install {name} oci://ghcr.io/vishal-770/charts/logrix -f values-aws.yaml"
            );
        }
        InfraProfile::CustomByo => {
            println!("  cd {name}");
            println!("  # Edit .env with your database and queue credentials");
            println!("  logrix all-in-one");
        }
    }
    println!("\n🌐 GraphQL API will be available at http://localhost:4000/\n");

    Ok(())
}

fn prompt_with_default(prompt: &str, default: &str) -> String {
    print!("{prompt} [{default}]: ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    let stdin = io::stdin();
    if stdin.lock().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    default.to_string()
}

fn parse_profile(opt: Option<&str>, non_interactive: bool) -> InfraProfile {
    if let Some(p) = opt {
        match p.to_lowercase().as_str() {
            "aws" | "terraform" => return InfraProfile::AwsTerraform,
            "byo" | "custom" => return InfraProfile::CustomByo,
            _ => return InfraProfile::LocalDocker,
        }
    }
    if non_interactive {
        return InfraProfile::LocalDocker;
    }
    println!("Select Infrastructure Profile:");
    println!("  1) Local Docker (PostgreSQL 16, RabbitMQ with UI, MinIO S3) [Recommended]");
    println!("  2) AWS Cloud-Native (Automated Terraform for Aurora, SQS, S3, EKS)");
    println!("  3) Custom / Bring Your Own Infrastructure (Existing DB/Queue)");
    let choice = prompt_with_default("Choice", "1");
    match choice.as_str() {
        "2" => InfraProfile::AwsTerraform,
        "3" => InfraProfile::CustomByo,
        _ => InfraProfile::LocalDocker,
    }
}

fn parse_network(opt: Option<&str>, non_interactive: bool) -> (u64, String) {
    if let Some(n) = opt {
        match n.to_lowercase().as_str() {
            "eth" | "ethereum" | "1" => (1, "https://eth.llamarpc.com".into()),
            "base" | "8453" => (8453, "https://mainnet.base.org".into()),
            "polygon" | "137" => (137, "https://polygon-rpc.com".into()),
            "arbitrum-sepolia" | "421614" => {
                (421614, "https://sepolia-rollup.arbitrum.io/rpc".into())
            }
            _ => (42161, "https://arb1.arbitrum.io/rpc".into()),
        }
    } else if non_interactive {
        (42161, "https://arb1.arbitrum.io/rpc".into())
    } else {
        println!("Select Blockchain Network:");
        println!("  1) Arbitrum One (Chain ID 42161) [Default]");
        println!("  2) Ethereum Mainnet (Chain ID 1)");
        println!("  3) Base (Chain ID 8453)");
        println!("  4) Polygon (Chain ID 137)");
        println!("  5) Arbitrum Sepolia Testnet (Chain ID 421614)");
        let choice = prompt_with_default("Choice", "1");
        match choice.as_str() {
            "2" => (1, "https://eth.llamarpc.com".into()),
            "3" => (8453, "https://mainnet.base.org".into()),
            "4" => (137, "https://polygon-rpc.com".into()),
            "5" => (421614, "https://sepolia-rollup.arbitrum.io/rpc".into()),
            _ => (42161, "https://arb1.arbitrum.io/rpc".into()),
        }
    }
}

fn parse_logic(opt: Option<&str>, non_interactive: bool) -> LogicStrategy {
    if let Some(l) = opt {
        if l.eq_ignore_ascii_case("wasm") || l.eq_ignore_ascii_case("typescript") {
            return LogicStrategy::TypeScriptWasm;
        }
        return LogicStrategy::Declarative;
    }
    if non_interactive {
        return LogicStrategy::Declarative;
    }
    println!("Select Indexing Strategy:");
    println!("  1) Declarative YAML (Zero code, config-based extraction) [Recommended]");
    println!("  2) TypeScript / WASM Handlers (Custom business logic & state mutation)");
    let choice = prompt_with_default("Choice", "1");
    if choice == "2" {
        LogicStrategy::TypeScriptWasm
    } else {
        LogicStrategy::Declarative
    }
}
