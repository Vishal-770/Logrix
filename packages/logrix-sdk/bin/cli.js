#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const readline = require("readline");

const args = process.argv.slice(2);
const command = args[0] || "help";

function showHelp() {
  console.log(`
Logrix CLI - Developer Platform for Blockchain Indexers

Usage:
  npx @logrix/sdk init [project-name]   Scaffold a new blockchain indexer project
  npx @logrix/sdk codegen               Generate TypeScript types from ABI and schema
  npx @logrix/sdk help                  Show help information
`);
}

function createPrompter() {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const it = rl[Symbol.asyncIterator]();

  return {
    async prompt(question, defaultValue) {
      process.stdout.write(`${question} [${defaultValue}]: `);
      const res = await it.next();
      if (res.done) return defaultValue;
      return res.value.trim() || defaultValue;
    },
    close() {
      rl.close();
    }
  };
}

const PRESET_NETWORKS = {
  "1": { name: "Ethereum Mainnet", chainId: 1, rpc: "https://eth.llamarpc.com" },
  "2": { name: "Arbitrum One", chainId: 42161, rpc: "https://arb1.arbitrum.io/rpc" },
  "3": { name: "Base", chainId: 8453, rpc: "https://mainnet.base.org" },
  "4": { name: "Polygon", chainId: 137, rpc: "https://polygon-rpc.com" },
  "5": { name: "Arbitrum Sepolia", chainId: 421614, rpc: "https://sepolia-rollup.arbitrum.io/rpc" },
};

const STANDARD_ERC20_ABI = [
  {
    anonymous: false,
    inputs: [
      { indexed: true, name: "from", type: "address" },
      { indexed: true, name: "to", type: "address" },
      { indexed: false, name: "value", type: "uint256" },
    ],
    name: "Transfer",
    type: "event",
  },
  {
    anonymous: false,
    inputs: [
      { indexed: true, name: "owner", type: "address" },
      { indexed: true, name: "spender", type: "address" },
      { indexed: false, name: "value", type: "uint256" },
    ],
    name: "Approval",
    type: "event",
  },
];

async function runInit() {
  console.log("\nLogrix Project Scaffolder");
  console.log("=========================\n");

  const prompter = createPrompter();

  const defaultName = args[1] || "my-indexer";
  const projectName = await prompter.prompt("Project name", defaultName);
  const targetDir = path.resolve(process.cwd(), projectName);

  if (fs.existsSync(targetDir)) {
    console.error(`Error: Directory '${projectName}' already exists.`);
    prompter.close();
    process.exit(1);
  }

  console.log("\nSelect Network:");
  console.log("  1) Ethereum Mainnet (Chain ID 1)");
  console.log("  2) Arbitrum One (Chain ID 42161)");
  console.log("  3) Base (Chain ID 8453)");
  console.log("  4) Polygon (Chain ID 137)");
  console.log("  5) Arbitrum Sepolia Testnet (Chain ID 421614)");
  const netChoice = await prompter.prompt("Select network number", "2");
  const selectedNet = PRESET_NETWORKS[netChoice] || PRESET_NETWORKS["2"];

  const contractName = await prompter.prompt("Smart Contract Name", "TokenContract");
  const contractAddress = await prompter.prompt(
    "Target Contract Address",
    "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
  );
  const startBlock = await prompter.prompt("Start Block", "0");
  prompter.close();

  console.log(`\nScaffolding project in ${targetDir}...`);
  fs.mkdirSync(targetDir, { recursive: true });
  fs.mkdirSync(path.join(targetDir, "abis"), { recursive: true });
  fs.mkdirSync(path.join(targetDir, "handlers"), { recursive: true });

  const abiFile = path.join(targetDir, "abis", `${contractName}.json`);
  fs.writeFileSync(abiFile, JSON.stringify(STANDARD_ERC20_ABI, null, 2));

  const manifestContent = `version: "0.1.0"
network:
  chain_id: ${selectedNet.chainId}
  rpc_url: "${selectedNet.rpc}"

contracts:
  - name: "${contractName}"
    address: "${contractAddress}"
    start_block: ${startBlock}
    wasm_handler: "./handlers/mapping.wasm"
    events:
      - "Transfer(address indexed from, address indexed to, uint256 value)"
      - "Approval(address indexed owner, address indexed spender, uint256 value)"
`;
  fs.writeFileSync(path.join(targetDir, "manifest.yaml"), manifestContent);

  const schemaContent = `type Transfer @entity {
  id: ID!
  blockNumber: BigInt!
  fromAddress: String! @index
  toAddress: String! @index
  amount: String!
  transactionHash: String!
  timestamp: BigInt!
}
`;
  fs.writeFileSync(path.join(targetDir, "schema.graphql"), schemaContent);

  const packageJson = {
    name: projectName,
    version: "0.1.0",
    private: true,
    scripts: {
      codegen: "logrix codegen",
      build: "asc handlers/mapping.ts -o handlers/mapping.wasm --optimize --exportRuntime",
      test: "asc handlers/mapping.ts --noEmit",
    },
    dependencies: {
      "@logrix/sdk": "^0.1.0",
    },
    devDependencies: {
      assemblyscript: "^0.27.29",
    },
  };
  fs.writeFileSync(
    path.join(targetDir, "package.json"),
    JSON.stringify(packageJson, null, 2)
  );

  const tsconfig = {
    extends: "assemblyscript/std/assembly.json",
    include: ["./handlers/**/*.ts"],
  };
  fs.writeFileSync(
    path.join(targetDir, "tsconfig.json"),
    JSON.stringify(tsconfig, null, 2)
  );

  const mappingTs = `import { TransferEvent } from "./generated/events";
import { TransferEntity } from "./generated/schema";

export function handleTransfer(event: TransferEvent): void {
  let entity = new TransferEntity(event.transactionHash + "-" + event.logIndex.toString());
  entity.blockNumber = event.blockNumber;
  entity.fromAddress = event.params.from;
  entity.toAddress = event.params.to;
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();
}
`;
  fs.writeFileSync(path.join(targetDir, "handlers", "mapping.ts"), mappingTs);

  const valuesLocal = `# Local Kubernetes Profile (Minikube / Kind / K3s)
# 1-click deployment with built-in PostgreSQL 16 and RabbitMQ

image:
  repository: ghcr.io/vishal-770/logrix
  tag: "0.1.0"
  pullPolicy: IfNotPresent

localDev:
  enabled: true

config:
  chainId: ${selectedNet.chainId}
  contractAddress: "${contractAddress}"
  rpcUrl: "${selectedNet.rpc}"
  queueDriver: "rabbitmq"
  rabbitmqUrl: "amqp://logrix:logrixpassword@logrix-rabbitmq:5672/%2f"
  databaseUrl: "postgres://logrix:logrixpassword@logrix-postgres:5432/logrix"
  reconcilerIntervalSecs: 15
  ringBufferDepth: 64

ingester:
  replicas: 1
  leaderElection:
    enabled: false

processor:
  replicas: 1
  keda:
    enabled: false

api:
  replicas: 1
  hpa:
    enabled: false
  service:
    type: NodePort
    port: 4000
`;
  fs.writeFileSync(path.join(targetDir, "values-local.yaml"), valuesLocal);

  const valuesAws = `# Production AWS EKS Profile
# Provisions on AWS Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3

image:
  repository: ghcr.io/vishal-770/logrix
  tag: "0.1.0"
  pullPolicy: IfNotPresent

localDev:
  enabled: false

config:
  chainId: ${selectedNet.chainId}
  contractAddress: "${contractAddress}"
  rpcUrl: "${selectedNet.rpc}"
  queueDriver: "sqs"
  reconcilerIntervalSecs: 30
  ringBufferDepth: 256
  databaseUrl: "postgres://admin:YOUR_PASSWORD@your-aurora-cluster.rds.amazonaws.com:5432/logrix?sslmode=require"
  sqsQueueUrl: "https://sqs.us-east-1.amazonaws.com/123456789012/${projectName}-queue"
  s3Bucket: "${projectName}-cold-blobs"

serviceAccount:
  create: true
  name: "logrix-sa"
  annotations:
    eks.amazonaws.com/role-arn: "arn:aws:iam::123456789012:role/${projectName}-eks-role"

ingester:
  replicas: 2
  leaderElection:
    enabled: true
    leaseName: "logrix-ingester-lease"

processor:
  replicas: 2
  keda:
    enabled: true
    minReplicaCount: 2
    maxReplicaCount: 50
    queueType: sqs
    targetQueueDepth: 100

api:
  replicas: 2
  hpa:
    enabled: true
    minReplicas: 2
    maxReplicas: 20
    targetCPUUtilizationPercentage: 75
  service:
    type: ClusterIP
    port: 4000
`;
  fs.writeFileSync(path.join(targetDir, "values-aws.yaml"), valuesAws);

  const readmeContent = `# ${projectName}

Blockchain indexer project built with Logrix.

## Development Workflow

### 1. Install Dependencies
\`\`\`bash
npm install
\`\`\`

### 2. Generate TypeScript Bindings from ABI and Schema
\`\`\`bash
npm run codegen
\`\`\`

### 3. Compile WebAssembly Handlers
\`\`\`bash
npm run build
\`\`\`

---

## Deploy to Kubernetes

Deploy your indexer cluster directly with Helm:

### Option A: Local Kubernetes (Minikube / Kind / K3s)
\`\`\`bash
helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
  -f values-local.yaml \\
  --set-file config.manifestContent=manifest.yaml \\
  --set-file config.schemaContent=schema.graphql \\
  --set-file config.wasmBinary=handlers/mapping.wasm
\`\`\`

Port forward to access the GraphQL Playground:
\`\`\`bash
kubectl port-forward svc/${projectName}-logrix-api 4000:4000
\`\`\`
Open http://localhost:4000/

### Option B: Production AWS EKS
\`\`\`bash
helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
  -f values-aws.yaml \\
  --set-file config.manifestContent=manifest.yaml \\
  --set-file config.schemaContent=schema.graphql \\
  --set-file config.wasmBinary=handlers/mapping.wasm
\`\`\`
`;
  fs.writeFileSync(path.join(targetDir, "README.md"), readmeContent);

  // Generate initial bindings
  runCodegenInDir(targetDir);

  console.log(`\nProject successfully created in ${projectName}/!`);
  console.log("\nNext steps:");
  console.log(`  cd ${projectName}`);
  console.log("  npm install");
  console.log("  npm run build\n");
}

function runCodegenInDir(projectDir) {
  const abisDir = path.join(projectDir, "abis");
  const generatedDir = path.join(projectDir, "handlers", "generated");
  fs.mkdirSync(generatedDir, { recursive: true });

  const eventsCode = generateEventsCode(abisDir);
  fs.writeFileSync(path.join(generatedDir, "events.ts"), eventsCode);

  const schemaFile = path.join(projectDir, "schema.graphql");
  const schemaCode = generateSchemaCode(schemaFile);
  fs.writeFileSync(path.join(generatedDir, "schema.ts"), schemaCode);
}

function generateEventsCode(abisDir) {
  let code = `// Auto-generated by Logrix Codegen. Do not edit manually.
import { EventLog } from "@logrix/sdk";

`;

  if (!fs.existsSync(abisDir)) {
    return code;
  }

  const abiFiles = fs.readdirSync(abisDir).filter((f) => f.endsWith(".json"));
  for (const file of abiFiles) {
    try {
      const content = fs.readFileSync(path.join(abisDir, file), "utf-8");
      const abi = JSON.parse(content);
      const events = Array.isArray(abi) ? abi.filter((x) => x.type === "event") : [];

      for (const ev of events) {
        const className = `${ev.name}Event`;
        const paramsClassName = `${ev.name}Params`;

        code += `export class ${paramsClassName} {\n`;
        const inputs = ev.inputs || [];
        for (const input of inputs) {
          code += `  ${input.name}: string = "";\n`;
        }

        code += `\n  constructor(raw: EventLog) {\n`;
        let indexedIdx = 1;
        for (const input of inputs) {
          if (input.indexed) {
            code += `    this.${input.name} = raw.topics.length > ${indexedIdx} ? raw.topics[${indexedIdx}] : "";\n`;
            indexedIdx++;
          } else {
            code += `    this.${input.name} = raw.data;\n`;
          }
        }
        code += `  }\n}\n\n`;

        code += `export class ${className} {\n`;
        code += `  address: string;\n`;
        code += `  blockNumber: u64;\n`;
        code += `  blockHash: string;\n`;
        code += `  transactionHash: string;\n`;
        code += `  logIndex: u32;\n`;
        code += `  blockTimestamp: u64;\n`;
        code += `  params: ${paramsClassName};\n\n`;

        code += `  constructor(raw: EventLog) {\n`;
        code += `    this.address = raw.address;\n`;
        code += `    this.blockNumber = raw.block_number;\n`;
        code += `    this.blockHash = raw.block_hash;\n`;
        code += `    this.transactionHash = raw.transaction_hash;\n`;
        code += `    this.logIndex = raw.log_index;\n`;
        code += `    this.blockTimestamp = raw.block_timestamp;\n`;
        code += `    this.params = new ${paramsClassName}(raw);\n`;
        code += `  }\n}\n\n`;
      }
    } catch (e) {
      console.warn(`Warning: failed to parse ABI in ${file}: ${e.message}`);
    }
  }

  return code;
}

function generateSchemaCode(schemaPath) {
  let code = `// Auto-generated by Logrix Codegen. Do not edit manually.
import { logrix_emit, logrix_db_get, logrix_db_set } from "@logrix/sdk";

`;

  if (!fs.existsSync(schemaPath)) {
    return code;
  }

  const content = fs.readFileSync(schemaPath, "utf-8");
  const entityRegex = /type\s+(\w+)\s+@entity\s*\{([^}]+)\}/g;
  let match;

  while ((match = entityRegex.exec(content)) !== null) {
    const entityName = match[1];
    const body = match[2];
    const lines = body.split("\n");

    const fields = [];
    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line || line.startsWith("#")) continue;
      const parts = line.split(":");
      if (parts.length >= 2) {
        const fieldName = parts[0].trim();
        const fieldType = parts[1].replace(/@\w+/g, "").trim();
        fields.push({ name: fieldName, type: fieldType });
      }
    }

    code += `export class ${entityName}Entity {\n`;
    for (const f of fields) {
      if (f.name === "id") {
        code += `  id: string;\n`;
      } else if (f.type.startsWith("BigInt") || f.type.startsWith("Int")) {
        code += `  ${f.name}: u64 = 0;\n`;
      } else {
        code += `  ${f.name}: string = "";\n`;
      }
    }

    code += `\n  constructor(id: string) {\n`;
    code += `    this.id = id;\n`;
    code += `  }\n\n`;

    code += `  save(): void {\n`;
    code += `    let payload = "{"`;
    let first = true;
    for (const f of fields) {
      const prefix = first ? `+ "\\"${f.name}\\":"` : `+ ",\\"${f.name}\\":"`;
      first = false;
      if (f.type.startsWith("BigInt") || f.type.startsWith("Int")) {
        code += ` ${prefix} + this.${f.name}.toString()`;
      } else {
        code += ` ${prefix} + "\\"" + this.${f.name} + "\\""`;
      }
    }
    code += ` + "}";\n`;
    code += `    logrix_emit("${entityName}", payload);\n`;
    code += `  }\n}\n\n`;
  }

  return code;
}

function runCodegen() {
  const projectDir = process.cwd();
  console.log("Generating Logrix bindings for project in:", projectDir);
  runCodegenInDir(projectDir);
  console.log("Success! Generated handlers/generated/events.ts and handlers/generated/schema.ts");
}

if (command === "init") {
  runInit();
} else if (command === "codegen") {
  runCodegen();
} else {
  showHelp();
}
