pub fn docker_compose_template() -> &'static str {
    r#"version: "3.8"
services:
  postgres:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: logrix
      POSTGRES_PASSWORD: logrixpassword
      POSTGRES_DB: logrix
    ports: ["5432:5432"]
    volumes: [pg_data:/var/lib/postgresql/data]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U logrix -d logrix"]
      interval: 5s
      timeout: 5s
      retries: 5

  rabbitmq:
    image: rabbitmq:3.13-management-alpine
    restart: unless-stopped
    environment:
      RABBITMQ_DEFAULT_USER: logrix
      RABBITMQ_DEFAULT_PASS: logrixpassword
    ports:
      - "5672:5672"
      - "15672:15672"
    volumes: [rabbitmq_data:/var/lib/rabbitmq]

  minio:
    image: minio/minio:RELEASE.2024-03-30T09-41-56Z
    restart: unless-stopped
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: logrix
      MINIO_ROOT_PASSWORD: logrixpassword
    ports: ["9000:9000", "9001:9001"]
    volumes: [minio_data:/data]

volumes:
  pg_data:
  rabbitmq_data:
  minio_data:
"#
}

pub fn schema_graphql_template() -> &'static str {
    r#"type Transfer @entity {
  id: ID!
  blockNumber: BigInt!
  fromAddress: String! @index
  toAddress: String! @index
  amount: BigInt!
  transactionHash: String!
  timestamp: BigInt!
}
"#
}

pub fn ts_handler_template() -> &'static str {
    r#"import { EventLog, logrix_db_get, logrix_db_set, logrix_emit } from "@logrix/sdk";

export function handleTransfer(event: EventLog): void {
  logrix_emit("Transfer", {
    id: event.transaction_hash + "-" + event.log_index.toString(),
    blockNumber: event.block_number,
    fromAddress: event.topics[1],
    toAddress: event.topics[2],
    amount: event.data,
    transactionHash: event.transaction_hash,
    timestamp: event.block_timestamp
  });
}
"#
}

pub fn package_json_template(name: &str) -> String {
    format!(
        r#"{{
  "name": "{name}-indexer",
  "version": "0.1.0",
  "private": true,
  "scripts": {{
    "build": "asc handlers/mapping.ts -o handlers/mapping.wasm --optimize --exportRuntime",
    "deploy": "logrix deploy"
  }},
  "dependencies": {{
    "@logrix/sdk": "^0.1.0"
  }},
  "devDependencies": {{
    "assemblyscript": "^0.27.29"
  }}
}}
"#
    )
}

pub fn tsconfig_template() -> &'static str {
    r#"{
  "extends": "assemblyscript/std/assembly.json",
  "include": [
    "./handlers/**/*.ts"
  ]
}
"#
}

pub fn sdk_types_template() -> &'static str {
    r#"// @logrix/sdk WebAssembly Host Interface
export class EventLog {
  address: string = "";
  block_number: u64 = 0;
  block_hash: string = "";
  transaction_hash: string = "";
  log_index: u32 = 0;
  block_timestamp: u64 = 0;
  topics: Array<string> = [];
  data: string = "";
}

@external("env", "logrix_emit")
declare function host_emit(ptr: usize, len: usize): void;

@external("env", "logrix_db_get")
declare function host_db_get(ptr: usize, len: usize): usize;

@external("env", "logrix_db_set")
declare function host_db_set(k_ptr: usize, k_len: usize, v_ptr: usize, v_len: usize): void;

export function logrix_emit(entityType: string, jsonPayload: string): void {
  // Stub for local build check
}
"#
}

pub fn readme_startup_steps(is_docker: bool, is_aws: bool) -> &'static str {
    if is_docker {
        "### 1-Click Local Deploy\n```bash\nlogrix deploy --local\n```\nThis automatically compiles your TypeScript handlers and starts PostgreSQL, RabbitMQ, and the indexer."
    } else if is_aws {
        "### 1-Click AWS Production Deploy\n```bash\nlogrix deploy --aws\n```\nThis guides you through provisioning Aurora PostgreSQL, Amazon SQS, and deploying the Logrix Helm chart to AWS EKS."
    } else {
        "### 1. Configure .env credentials\nEdit `.env` to supply your target connection strings.\n\n### 2. Run Indexer\n```bash\nlogrix all-in-one\n```"
    }
}
