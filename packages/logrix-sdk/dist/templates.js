"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.ERC20_ABI = void 0;
exports.logrixYamlTemplate = logrixYamlTemplate;
exports.schemaGraphqlTemplate = schemaGraphqlTemplate;
exports.mappingTemplate = mappingTemplate;
exports.packageJsonTemplate = packageJsonTemplate;
exports.tsconfigTemplate = tsconfigTemplate;
exports.gitignoreTemplate = gitignoreTemplate;
exports.valuesAwsTemplate = valuesAwsTemplate;
exports.secretsExampleTemplate = secretsExampleTemplate;
exports.readmeTemplate = readmeTemplate;
// Template generators for `logrix init` scaffolding.
const yaml_1 = require("yaml");
function logrixYamlTemplate(a) {
    const doc = {
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
    return (0, yaml_1.stringify)(doc, { indent: 2 });
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
            "@logrix/sdk": "^0.6.0",
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
function valuesAwsTemplate(a) {
    const hasWebhooks = Boolean(a.webhookUrl);
    const doc = {
        image: {
            repository: "ghcr.io/vishal-770/logrix",
            tag: "0.6.0",
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
    return (0, yaml_1.stringify)(doc, { indent: 2 });
}
function secretsExampleTemplate(a) {
    const secretKeys = new Set([
        "RPC_URL",
        "RPC_FALLBACK_URLS",
        "DATABASE_URL",
        ...(a.secretEnvVars ?? []),
    ]);
    const doc = {
        apiVersion: "v1",
        kind: "Secret",
        metadata: {
            name: "logrix-secrets",
            namespace: "default",
        },
        type: "Opaque",
        stringData: Object.fromEntries([...secretKeys].map((key) => [key, "CHANGE_ME"])),
    };
    return (0, yaml_1.stringify)(doc, { indent: 2 });
}
function readmeTemplate(projectName) {
    return `# ${projectName}

Blockchain indexer built with [Logrix](https://github.com/Vishal-770/Logrix).

## Develop and export

Run commands from this project directory after reviewing the contract address,
start block, RPC URLs, ABI, and event mappings in \`logrix.yaml\`:

\`\`\`bash
npm install
npm run codegen
\`\`\`

Current SDK compilation workaround: change imports/re-exports of
\`"@logrix/sdk"\` to \`"@logrix/sdk/src/runtime/index"\` in \`src/mapping.ts\`,
\`src/generated/events.ts\`, and \`src/generated/schema.ts\`. The root currently
re-exports testing source that \`asc\` cannot parse (optional properties/nullish
coalescing). Repeat the generated-file adjustment after each codegen. Then:

\`\`\`bash
npm run validate
npm run build:asc
npm run export-values
\`\`\`

The scaffold always starts with ERC-20 Transfer/Approval ABI and handlers. A
different address/name does not fetch a different ABI. After ABI/schema edits,
regenerate bindings and adapt the handler yourself. \`add contract\` can copy a
local ABI with \`--abi <path>\` or try \`--fetch-abi\` (falls back to ERC-20 on
failure); it does not write handler code. See the
[SDK CLI reference](https://github.com/Vishal-770/Logrix/blob/main/packages/logrix-sdk/README.md#cli-reference)
for all flags and limitations.

### Files and scripts

- Source: \`logrix.yaml\`, \`schema.graphql\`, \`abis/\`, \`src/mapping.ts\`.
- \`codegen\`: overwrites \`src/generated/events.ts\` and \`src/generated/schema.ts\`.
- \`validate\`: checks manifest/file shape; does not compile or prove ABI/handler correctness.
- \`build:asc\`: compiles \`src/mapping.ts\` to \`build/mapping.wasm\`.
- \`build\`: invokes \`logrix build\`; currently an existing first \`mapping.file\`
  overrides its compiler input, so repeated builds can try compiling the output
  WASM as source. Use \`build:asc\` for reliable rebuilds with this layout.
- \`export-values\`: embeds the schema, converted runtime manifest, and base64 WASM
  into \`indexer-values.yaml\`. It always reads \`build/mapping.wasm\`, regardless
  of \`mapping.file\` or \`build --out\`. All exported contracts share the mounted
  \`/etc/logrix/handlers/mapping.wasm\`.

The generated package has no \`test\` script. Optional \`npx logrix test\` checks
basic files, compiles \`build/test_mapping.wasm\`, and invokes \`node --test\` if
tests are present; it neither generates bindings nor executes WASM handlers.
Its \`--watch\` flag has no effect, and no TypeScript/AssemblyScript Node loader
is supplied. \`npx logrix status\` probes live RPC endpoints rather than runtime
database progress; it does not expand environment placeholders and prints URLs.

### Handler language and ABI

Handler files are AssemblyScript, compiled by \`asc\`, not ordinary Node.js or
browser TypeScript. The host writes UTF-8 event JSON into exported \`memory\`,
uses exported \`allocate(size)\`, and calls \`handle_event(ptr, len) -> i32\`.
Keep the allocator export. Zero succeeds; a negative result or trap fails the
invocation. The SDK uses \`env.logrix_db_get/set/remove\`, \`env.logrix_emit\`,
and \`env.logrix_log\`; the host also registers them under \`logrix\` and supplies
\`env.abort\`. State/entity changes are staged for the Rust processor.

Parse with \`EventLog.fromJson\` and dispatch by topic0 (and address for multiple
contracts). The host does not call named \`eventHandlers.handler\` functions.
Generated event classes decode ABI fields; generated entity classes expose
\`load\`, \`remove\`, and \`save\`. Review decoder limitations in the SDK README:
signed Solidity integers are not sign-decoded, BigInt arithmetic assumes
non-negative normalized inputs, and entity JSON escaping/types are limited.
Current Rust event JSON has no block timestamp, so \`event.blockTimestamp\`
defaults to zero; numeric metadata also uses an \`i32\` intermediate in parsing.

## Configuration and secrets

Helm export maps network/start-block settings and supported \`indexing\` keys to
chart \`config\` values, which become process environment variables. Infrastructure
(DB/queue URLs, image, S3, replicas, KEDA) needs separate values/Secrets.
Direct Rust \`MANIFEST_PATH=logrix.yaml\` loading does not configure process RPC,
chain ID, start block, schema path, or all tuning fields from this SDK manifest.
See the [configuration reference](https://github.com/Vishal-770/Logrix/blob/main/docs/configuration.md).

Literal placeholders such as \`\${ALCHEMY_API_KEY}\` stay symbolic in export;
the CLI does not read \`.env\`. At startup the Rust RPC gateway and manifest
loader expand references from the process environment. Missing/invalid references
are configuration errors on those paths. Webhook secrets written as a pure
\`\${WEBHOOK_SECRET}\` reference are preserved; literal/mixed secrets become
\`\${WEBHOOK_SECRET}\` (or numbered references for later subscriptions).

References cause export to set \`config.existingSecret\` (default \`logrix-secrets\`).
New \`deploy/secrets.example.yaml\` files contain \`CHANGE_ME\` entries for RPC,
fallback RPC, DB, and discovered names. Prepare a private Secret in the release
namespace. Secret env keys override chart ConfigMap keys: remove keys intended
to inherit from the ConfigMap, or set their full intended value. Do not apply
untouched examples or commit populated credentials. Existing examples stay
unchanged unless \`npx logrix export-values --deploy-profiles\` is used, which
overwrites both \`deploy/values-aws.yaml\` and \`deploy/secrets.example.yaml\`.
Neither init nor export creates \`deploy/values-local.yaml\` despite CLI help text.

## Deployment handoff

Use Helm/Kubernetes/Terraform separately with pre-built, versioned runtime images.
Review the chart's backing services and autoscaling prerequisites before applying
\`indexer-values.yaml\`; the default chart enables KEDA and needs its CRDs or an
override. The AWS profile has example account/role/SQS identifiers to replace;
it does not provision infrastructure.

Current source-chart blocker: export sets \`MANIFEST_PATH\`, but the chart does
not mount the inline manifest into the ingester. That process reads the file at
startup and fails when absent. Supply a consumer-managed ingester ConfigMap
volume/mount patch or equivalent readable manifest before deployment; the
exported bundle is not deployable unchanged with this chart/runtime combination.

Versions differ: the Rust workspace is 0.7.3, SDK package 0.7.0, CLI version output
0.6.0, generated SDK dependency \`^0.6.0\`, and generated AWS image tag \`0.6.0\`.
Check the installed lockfile and explicitly select a tested runtime image.

- [Helm chart instructions](https://github.com/Vishal-770/Logrix/blob/main/deploy/helm/logrix/README.md)
- [Raw Kubernetes instructions](https://github.com/Vishal-770/Logrix/blob/main/deploy/k8s/README.md)
- [AWS Terraform instructions](https://github.com/Vishal-770/Logrix/blob/main/deploy/terraform/aws/README.md)
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
