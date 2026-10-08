"use strict";
// Template generators for `logrix init` scaffolding.
// Each function returns the file content string for a given template.
Object.defineProperty(exports, "__esModule", { value: true });
exports.ERC20_ABI = void 0;
exports.logrixYamlTemplate = logrixYamlTemplate;
exports.schemaGraphqlTemplate = schemaGraphqlTemplate;
exports.mappingTemplate = mappingTemplate;
exports.packageJsonTemplate = packageJsonTemplate;
exports.tsconfigTemplate = tsconfigTemplate;
exports.gitignoreTemplate = gitignoreTemplate;
exports.valuesLocalTemplate = valuesLocalTemplate;
exports.valuesAwsTemplate = valuesAwsTemplate;
exports.secretsExampleTemplate = secretsExampleTemplate;
exports.readmeTemplate = readmeTemplate;
function logrixYamlTemplate(a) {
    return `specVersion: "0.1.0"

network:
  name: "${a.networkName}"
  chainId: ${a.chainId}
  rpcUrl: "${a.rpcUrl}"

dataSources:
  - kind: ethereum/contract
    name: "${a.contractName}"
    network: "${a.networkName}"
    source:
      address: "${a.contractAddress}"
      abi: "${a.contractName}"
      startBlock: ${a.startBlock}
    mapping:
      kind: wasm/assemblyscript
      file: ./build/mapping.wasm
      abis:
        - name: "${a.contractName}"
          file: ./abis/${a.contractName}.json
      eventHandlers:
        - event: "Transfer(address indexed,address indexed,uint256)"
          handler: handleTransfer
        - event: "Approval(address indexed,address indexed,uint256)"
          handler: handleApproval
`;
}
function schemaGraphqlTemplate() {
    return `type Transfer @entity {
  id: ID!
  blockNumber: BigInt!
  fromAddress: String! @index
  toAddress: String! @index
  amount: BigInt!
  transactionHash: String!
  timestamp: BigInt!
}

type Approval @entity {
  id: ID!
  blockNumber: BigInt!
  ownerAddress: String! @index
  spenderAddress: String! @index
  amount: BigInt!
  transactionHash: String!
  timestamp: BigInt!
}
`;
}
function mappingTemplate(contractName) {
    return `import { EventLog, log, BigInt, Address, allocate } from "@logrix/sdk";
import { TransferEvent, ApprovalEvent } from "./generated/events";
import { Transfer, Approval } from "./generated/schema";

// Required export so the Logrix Rust engine can allocate linear memory
export { allocate } from "@logrix/sdk";

// ---------------------------------------------------------------------------
// Entry point called by the Logrix engine for every matched event log.
// ---------------------------------------------------------------------------
export function handle_event(ptr: i32, len: i32): i32 {
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = load<u8>(ptr + i);
  }
  const raw = String.UTF8.decode(bytes.buffer);

  // Dispatch based on event signature topic
  if (raw.includes('"Transfer"') || raw.includes("0xddf252ad")) {
    return handleTransfer(raw);
  }
  if (raw.includes('"Approval"') || raw.includes("0x8c5be1e5")) {
    return handleApproval(raw);
  }

  log.info("handle_event: unhandled event, skipping");
  return 0;
}

function handleTransfer(raw: string): i32 {
  const logObj = new EventLog();
  logObj.transaction_hash = extractField(raw, "transaction_hash");
  logObj.block_number = u64(parseInt(extractField(raw, "block_number")) as i32);
  logObj.block_timestamp = u64(parseInt(extractField(raw, "block_timestamp")) as i32);

  const event = new TransferEvent(logObj);
  event.params.from = Address.fromString(extractField(raw, "from"));
  event.params.to = Address.fromString(extractField(raw, "to"));
  event.params.value = BigInt.fromString(extractField(raw, "value"));

  const entityId = event.transactionHash + "-" + event.logIndex.toString();
  let entity = Transfer.load(entityId);
  if (!entity) {
    entity = new Transfer(entityId);
  }

  entity.blockNumber = event.blockNumber;
  entity.fromAddress = event.params.from.toHexString();
  entity.toAddress = event.params.to.toHexString();
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();

  log.info("Indexed Transfer entity: " + entityId);
  return 0;
}

function handleApproval(raw: string): i32 {
  const logObj = new EventLog();
  logObj.transaction_hash = extractField(raw, "transaction_hash");
  logObj.block_number = u64(parseInt(extractField(raw, "block_number")) as i32);
  logObj.block_timestamp = u64(parseInt(extractField(raw, "block_timestamp")) as i32);

  const event = new ApprovalEvent(logObj);
  event.params.owner = Address.fromString(extractField(raw, "owner"));
  event.params.spender = Address.fromString(extractField(raw, "spender"));
  event.params.value = BigInt.fromString(extractField(raw, "value"));

  const entityId = event.transactionHash + "-" + event.logIndex.toString();
  let entity = Approval.load(entityId);
  if (!entity) {
    entity = new Approval(entityId);
  }

  entity.blockNumber = event.blockNumber;
  entity.ownerAddress = event.params.owner.toHexString();
  entity.spenderAddress = event.params.spender.toHexString();
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();

  log.info("Indexed Approval entity: " + entityId);
  return 0;
}

function extractField(json: string, key: string): string {
  const needle = '"' + key + '":';
  const idx = json.indexOf(needle);
  if (idx === -1) return "";
  let start = idx + needle.length;
  while (start < json.length && json.charAt(start) === " ") start++;
  if (start >= json.length) return "";
  const ch = json.charAt(start);
  if (ch === '"') {
    start++;
    const end = json.indexOf('"', start);
    if (end === -1) return "";
    return json.slice(start, end);
  } else {
    let end = start;
    while (end < json.length) {
      const c = json.charCodeAt(end);
      if (c === 44 || c === 125 || c === 93 || c === 32 || c === 10 || c === 13) break;
      end++;
    }
    return json.slice(start, end).trim();
  }
}
`;
}
function packageJsonTemplate(projectName) {
    return {
        name: projectName,
        version: "0.1.0",
        private: true,
        scripts: {
            codegen: "logrix codegen",
            build: "logrix build",
            validate: "logrix validate",
            "export-values": "logrix export-values",
            "build:asc": "asc src/mapping.ts --config tsconfig.json -o build/mapping.wasm --optimize --exportRuntime",
        },
        dependencies: {
            "@logrix/sdk": "^0.3.3",
        },
        devDependencies: {
            assemblyscript: "^0.27.29",
        },
    };
}
function tsconfigTemplate() {
    return {
        extends: "assemblyscript/std/assembly.json",
        include: ["src/**/*.ts"],
    };
}
function gitignoreTemplate() {
    return `node_modules/
build/
src/generated/
indexer-values.yaml
`;
}
function valuesLocalTemplate(a) {
    return `# Local Kubernetes Profile (Minikube / Kind / K3s)
# 1-click local testing with in-cluster PostgreSQL and RabbitMQ

image:
  repository: ghcr.io/vishal-770/logrix
  tag: "0.3.3"
  pullPolicy: IfNotPresent

localDev:
  enabled: true
  postgres:
    storage: 1Gi
  rabbitmq:
    storage: 1Gi

config:
  chainId: ${a.chainId}
  reconcilerIntervalSecs: 15
  ringBufferDepth: 64

ingester:
  replicas: 1
  leaderElection:
    enabled: false
  resources:
    requests:
      cpu: 100m
      memory: 128Mi
    limits:
      cpu: 500m
      memory: 512Mi

processor:
  replicas: 1
  keda:
    enabled: false
  resources:
    requests:
      cpu: 250m
      memory: 256Mi
    limits:
      cpu: 1000m
      memory: 1Gi

api:
  replicas: 1
  hpa:
    enabled: false
  port: 4000
`;
}
function valuesAwsTemplate(a) {
    return `# Production AWS EKS Profile
# Provisions on AWS Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3

image:
  repository: ghcr.io/vishal-770/logrix
  tag: "0.3.3"
  pullPolicy: IfNotPresent

localDev:
  enabled: false

config:
  chainId: ${a.chainId}
  queueDriver: "sqs"
  reconcilerIntervalSecs: 30
  ringBufferDepth: 256
  # Name of Kubernetes Secret containing DATABASE_URL and RPC credentials
  existingSecret: "logrix-aws-secrets"

serviceAccount:
  create: true
  name: "logrix-sa"
  annotations:
    # AWS IAM Roles for Service Accounts (IRSA) for SQS and S3 access
    eks.amazonaws.com/role-arn: "arn:aws:iam::ACCOUNT_ID:role/logrix-production-role"

ingester:
  replicas: 2
  leaderElection:
    enabled: true
    leaseName: "logrix-ingester-lease"
  resources:
    requests:
      cpu: 100m
      memory: 128Mi
    limits:
      cpu: 500m
      memory: 512Mi

processor:
  keda:
    enabled: true
    minReplicaCount: 1
    maxReplicaCount: 50
    queueType: sqs
    sqs:
      queueUrl: "https://sqs.us-east-1.amazonaws.com/ACCOUNT_ID/logrix-live-jobs"
      awsRegion: "us-east-1"
  resources:
    requests:
      cpu: 250m
      memory: 256Mi
    limits:
      cpu: 1000m
      memory: 1Gi

api:
  replicas: 2
  port: 4000
`;
}
function secretsExampleTemplate(a) {
    return `# Example Kubernetes Secret for production credentials
# Apply with: kubectl apply -f deploy/secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: logrix-aws-secrets
  namespace: default
type: Opaque
stringData:
  # Primary paid JSON-RPC endpoint
  RPC_URL: "${a.rpcUrl}"
  # Optional fallback JSON-RPC endpoints (comma-separated)
  RPC_FALLBACK_URLS: ""
  # Production database connection string (e.g. AWS Aurora PostgreSQL)
  DATABASE_URL: "postgres://user:password@aurora-cluster.internal:5432/logrix"
`;
}
function readmeTemplate(projectName) {
    return `# ${projectName}

Blockchain indexer built with [Logrix](https://github.com/Vishal-770/Logrix).

## Development Workflow

### 1. Install dependencies
\`\`\`bash
npm install
\`\`\`

### 2. Configure \`logrix.yaml\`
Set your chain, contract addresses, and event handlers.
To add additional contracts:
\`\`\`bash
npx logrix add contract AnotherContract --address 0x... --start-block 0
\`\`\`

### 3. Generate typed bindings
\`\`\`bash
npm run codegen
\`\`\`
Generates \`src/generated/events.ts\` and \`src/generated/schema.ts\`.

### 4. Validate configuration
\`\`\`bash
npm run validate
\`\`\`

### 5. Write handler logic
Implement your event indexing logic in \`src/mapping.ts\`.

### 6. Compile WebAssembly handler
\`\`\`bash
npm run build
\`\`\`
Produces \`build/mapping.wasm\`.

### 7. Export Helm deployment bundle
\`\`\`bash
npm run export-values
\`\`\`
Produces \`indexer-values.yaml\` bundling your WASM binary, GraphQL schema, and contract manifest.

---

## Deployment (Helm)

Deploy to any Kubernetes cluster using the generated \`indexer-values.yaml\` and pre-configured profiles in \`deploy/\`:

### Option A: Local Kubernetes (Minikube / Kind / K3s)
Deploys with in-cluster PostgreSQL and RabbitMQ:
\`\`\`bash
helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
  -f deploy/values-local.yaml \\
  -f indexer-values.yaml
\`\`\`

Port-forward and open the GraphQL playground:
\`\`\`bash
kubectl port-forward svc/${projectName}-logrix-api 4000:4000
\`\`\`
Visit **http://localhost:4000/** in your browser.

### Option B: Production AWS EKS
Deploys with Aurora Serverless PostgreSQL, Amazon SQS, and KEDA autoscaling:
1. Configure credentials in \`deploy/secrets.example.yaml\` and apply:
   \`\`\`bash
   kubectl apply -f deploy/secrets.example.yaml
   \`\`\`
2. Deploy Helm release:
   \`\`\`bash
   helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
     -f deploy/values-aws.yaml \\
     -f indexer-values.yaml
   \`\`\`

### Option C: Custom Kubernetes Cluster
\`\`\`bash
helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
  -f my-cluster-infra.yaml \\
  -f indexer-values.yaml
\`\`\`
`;
}
exports.ERC20_ABI = [
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
