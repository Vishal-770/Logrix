// Template generators for `logrix init` scaffolding.
import { stringify as stringifyYaml } from "yaml";

export interface InitAnswers {
  projectName: string;
  networkName: string;
  chainId: number;
  rpcUrl: string;
  fallbackRpcUrls?: string[];
  contractName: string;
  contractAddress: string;
  startBlock: number;
  webhookUrl?: string;
  webhookEvents?: string[];
  ringBufferDepth?: number;
  reconcilerIntervalSecs?: number;
  fuelLimit?: number;
  maxMemoryMb?: number;
}

export function logrixYamlTemplate(a: InitAnswers): string {
  const doc: any = {
    specVersion: "0.1.0",
    network: {
      name: a.networkName,
      chainId: a.chainId,
      rpcUrl: a.rpcUrl,
    },
    dataSources: [
      {
        kind: "ethereum/contract",
        name: a.contractName,
        network: a.networkName,
        source: {
          address: a.contractAddress,
          abi: a.contractName,
          startBlock: a.startBlock,
        },
        mapping: {
          kind: "wasm/assemblyscript",
          file: "./build/mapping.wasm",
          abis: [
            {
              name: a.contractName,
              file: `./abis/${a.contractName}.json`,
            },
          ],
          eventHandlers: [
            {
              event: "Transfer(address indexed,address indexed,uint256)",
              handler: "handleTransfer",
            },
            {
              event: "Approval(address indexed,address indexed,uint256)",
              handler: "handleApproval",
            },
          ],
        },
      },
    ],
  };

  if (a.fallbackRpcUrls && a.fallbackRpcUrls.length > 0) {
    doc.network.fallbackRpcUrls = a.fallbackRpcUrls;
  }

  if (a.fuelLimit) {
    doc.dataSources[0].mapping.fuelLimit = a.fuelLimit;
  }
  if (a.maxMemoryMb) {
    doc.dataSources[0].mapping.maxMemoryMb = a.maxMemoryMb;
  }

  if (a.webhookUrl) {
    doc.webhooks = [
      {
        url: a.webhookUrl,
        events: a.webhookEvents && a.webhookEvents.length > 0 ? a.webhookEvents : ["reorg", "Transfer"],
        maxRetries: 3,
      },
    ];
  }

  doc.indexing = {
    ringBufferDepth: a.ringBufferDepth ?? 128,
    reconcilerIntervalSecs: a.reconcilerIntervalSecs ?? 30,
  };

  return stringifyYaml(doc, { indent: 2 });
}

export function schemaGraphqlTemplate(): string {
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

export function mappingTemplate(contractName: string): string {
  return `import { EventLog, log, allocate } from "@logrix/sdk";
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
  const rawJson = String.UTF8.decode(bytes.buffer);
  const logObj = EventLog.fromJson(rawJson);

  // Dispatch based on topic0 event signature
  // Transfer(address,address,uint256) -> 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef
  if (logObj.topics.length > 0 && logObj.topics[0].startsWith("0xddf252ad")) {
    return handleTransfer(new TransferEvent(logObj));
  }
  // Approval(address,address,uint256) -> 0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925
  if (logObj.topics.length > 0 && logObj.topics[0].startsWith("0x8c5be1e5")) {
    return handleApproval(new ApprovalEvent(logObj));
  }

  log.info("handle_event: unhandled topic0, skipping");
  return 0;
}

export function handleTransfer(event: TransferEvent): i32 {
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

export function handleApproval(event: ApprovalEvent): i32 {
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
`;
}

export function packageJsonTemplate(projectName: string): object {
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
      "@logrix/sdk": "^0.4.0",
    },
    devDependencies: {
      assemblyscript: "^0.27.29",
    },
  };
}

export function tsconfigTemplate(): object {
  return {
    extends: "assemblyscript/std/assembly.json",
    include: ["src/**/*.ts"],
  };
}

export function gitignoreTemplate(): string {
  return `node_modules/
build/
src/generated/
indexer-values.yaml
`;
}

export function valuesAwsTemplate(a: InitAnswers): string {
  const hasWebhooks = Boolean(a.webhookUrl);
  const doc: any = {
    image: {
      repository: "ghcr.io/vishal-770/logrix",
      tag: "0.4.0",
      pullPolicy: "IfNotPresent",
    },
    builtIn: {
      enabled: false,
      postgres: false,
      rabbitmq: false,
      minio: false,
    },
    localDev: {
      enabled: false,
    },
    config: {
      chainId: a.chainId,
      queueDriver: "sqs",
      reconcilerIntervalSecs: a.reconcilerIntervalSecs ?? 30,
      ringBufferDepth: a.ringBufferDepth ?? 256,
      existingSecret: "logrix-secrets",
      s3Bucket: "",
    },
    serviceAccount: {
      create: true,
      name: "logrix-sa",
      annotations: {
        "eks.amazonaws.com/role-arn": "arn:aws:iam::ACCOUNT_ID:role/logrix-production-role",
      },
    },
    ingester: {
      replicas: 2,
      leaderElection: {
        enabled: true,
        leaseName: "logrix-ingester-lease",
      },
      resources: {
        requests: { cpu: "100m", memory: "128Mi" },
        limits: { cpu: "500m", memory: "512Mi" },
      },
    },
    processor: {
      keda: {
        enabled: true,
        minReplicaCount: 1,
        maxReplicaCount: 50,
        queueType: "sqs",
        sqs: {
          queueUrl: "https://sqs.us-east-1.amazonaws.com/ACCOUNT_ID/logrix-live-jobs",
          awsRegion: "us-east-1",
        },
      },
      resources: {
        requests: { cpu: "250m", memory: "256Mi" },
        limits: { cpu: "1000m", memory: "1Gi" },
      },
    },
    api: {
      replicas: 2,
      port: 4000,
    },
    webhook: {
      enabled: hasWebhooks,
      replicaCount: 1,
    },
  };

  return stringifyYaml(doc, { indent: 2 });
}

export function secretsExampleTemplate(a: InitAnswers): string {
  const fallbackUrlsStr =
    a.fallbackRpcUrls && a.fallbackRpcUrls.length > 0
      ? a.fallbackRpcUrls.join(",")
      : "";
  const doc = {
    apiVersion: "v1",
    kind: "Secret",
    metadata: {
      name: "logrix-secrets",
      namespace: "default",
    },
    type: "Opaque",
    stringData: {
      RPC_URL: a.rpcUrl,
      RPC_FALLBACK_URLS: fallbackUrlsStr,
      DATABASE_URL: "postgres://user:password@aurora-cluster.internal:5432/logrix",
    },
  };

  return stringifyYaml(doc, { indent: 2 });
}

export function readmeTemplate(projectName: string): string {
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

### Mode 1: Self-Hosted Deployment (Default)
Deploys out-of-the-box with built-in PostgreSQL and RabbitMQ pods. No extra infrastructure config needed:
\`\`\`bash
helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
  -f indexer-values.yaml
\`\`\`
*(Optional S3 cold block archiving can be enabled with \`--set builtIn.minio=true\`)*

*(To use an existing database or queue securely, configure credentials in \`deploy/secrets.example.yaml\`, run \`kubectl apply -f deploy/secrets.example.yaml\`, and pass \`--set config.existingSecret=logrix-secrets\`)*

Port-forward and open the GraphQL playground:
\`\`\`bash
kubectl port-forward svc/${projectName}-logrix-api 4000:4000
\`\`\`
Visit **http://localhost:4000/** in your browser.

### Mode 2: Full AWS Production (EKS + Terraform)
Deploys with Aurora Serverless PostgreSQL, Amazon SQS, and KEDA autoscaling:
1. *(Optional)* If provisioning AWS infrastructure from scratch, clone https://github.com/Vishal-770/Logrix and run \`deploy/terraform/aws\`.
2. Configure credentials in \`deploy/secrets.example.yaml\` and apply:
   \`\`\`bash
   kubectl apply -f deploy/secrets.example.yaml
   \`\`\`
3. Deploy Helm release:
   \`\`\`bash
   helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix \\
     -f deploy/values-aws.yaml \\
     -f indexer-values.yaml \\
     --set config.existingSecret=logrix-secrets
   \`\`\`
`;
}

export const ERC20_ABI = [
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
