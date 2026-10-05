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
  amount: String!
  transactionHash: String!
  timestamp: BigInt!
}

type Approval @entity {
  id: ID!
  blockNumber: BigInt!
  ownerAddress: String! @index
  spenderAddress: String! @index
  amount: String!
  transactionHash: String!
  timestamp: BigInt!
}
`;
}
function mappingTemplate(contractName) {
    return `import { EventLog, logrix_log, allocate } from "@logrix/sdk";
import { TransferEvent, ApprovalEvent } from "./generated/events";
import { TransferEntity, ApprovalEntity } from "./generated/schema";

// ---------------------------------------------------------------------------
// Entry point called by the Logrix engine for every matched event log.
// The event JSON is written into guest memory at ptr/len by the host.
// ---------------------------------------------------------------------------
export { allocate } from "@logrix/sdk";

export function handle_event(ptr: i32, len: i32): i32 {
  // Decode raw bytes from guest linear memory into a string.
  // The host writes raw UTF-8 bytes — use memory.slice, not changetype.
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = load<u8>(ptr + i);
  }
  const raw = String.UTF8.decode(bytes.buffer);

  // Determine which event fired by checking the first topic (event signature hash).
  // topic[0] is keccak256 of the event signature — compare the first 10 chars for brevity.
  if (raw.includes('"Transfer"') || raw.includes("0xddf252ad")) {
    return handleTransfer(raw);
  }
  if (raw.includes('"Approval"') || raw.includes("0x8c5be1e5")) {
    return handleApproval(raw);
  }

  logrix_log(1, "handle_event: unknown event, skipping");
  return 0;
}

function handleTransfer(raw: string): i32 {
  const log = new EventLog();
  // Logrix passes decoded event fields — parse the JSON manually or extend EventLog.
  // For production use, replace with a proper JSON parsing library (e.g. json-as).
  log.transaction_hash = extractField(raw, "transaction_hash");
  log.block_number = u64(parseInt(extractField(raw, "block_number")) as i32);
  log.block_timestamp = u64(parseInt(extractField(raw, "block_timestamp")) as i32);

  const event = new TransferEvent(log);
  // The 'from', 'to', and 'value' params come from the decoded ABI fields in the JSON.
  event.params.from = extractField(raw, "from");
  event.params.to = extractField(raw, "to");
  event.params.value = extractField(raw, "value");

  const entity = new TransferEntity(
    event.transactionHash + "-" + event.logIndex.toString()
  );
  entity.blockNumber = event.blockNumber;
  entity.fromAddress = event.params.from;
  entity.toAddress = event.params.to;
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();
  return 0;
}

function handleApproval(raw: string): i32 {
  const log = new EventLog();
  log.transaction_hash = extractField(raw, "transaction_hash");
  log.block_number = u64(parseInt(extractField(raw, "block_number")) as i32);
  log.block_timestamp = u64(parseInt(extractField(raw, "block_timestamp")) as i32);

  const event = new ApprovalEvent(log);
  event.params.owner = extractField(raw, "owner");
  event.params.spender = extractField(raw, "spender");
  event.params.value = extractField(raw, "value");

  const entity = new ApprovalEntity(
    event.transactionHash + "-" + event.logIndex.toString()
  );
  entity.blockNumber = event.blockNumber;
  entity.ownerAddress = event.params.owner;
  entity.spenderAddress = event.params.spender;
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();
  return 0;
}

// ---------------------------------------------------------------------------
// Minimal field extractor. Replace with json-as for production handlers.
// Extracts the value of "key":"value" or "key":number from a JSON string.
// ---------------------------------------------------------------------------
function extractField(json: string, key: string): string {
  const needle = '"' + key + '":';
  const idx = json.indexOf(needle);
  if (idx === -1) return "";
  let start = idx + needle.length;
  // Skip whitespace
  while (start < json.length && json.charAt(start) === " ") start++;
  if (start >= json.length) return "";
  const ch = json.charAt(start);
  if (ch === '"') {
    // String value
    start++;
    const end = json.indexOf('"', start);
    if (end === -1) return "";
    return json.slice(start, end);
  } else {
    // Numeric value
    let end = start;
    while (end < json.length) {
      const c = json.charCodeAt(end);
      if (c < 48 || c > 57) break; // not a digit
      end++;
    }
    return json.slice(start, end);
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
            "build:asc": "asc src/mapping.ts --config tsconfig.json -o build/mapping.wasm --optimize --exportRuntime",
        },
        dependencies: {
            "@logrix/sdk": "^0.1.0",
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
`;
}
function readmeTemplate(projectName) {
    return `# ${projectName}

Blockchain indexer built with [Logrix](https://github.com/Vishal-770/Logrix).

## Workflow

### 1. Install dependencies
\`\`\`bash
npm install
\`\`\`

### 2. Edit \`logrix.yaml\`
Configure your contract address, chain, start block, and event handlers.

### 3. Place your ABI
Drop the contract ABI JSON into \`abis/<ContractName>.json\`.

### 4. Generate typed bindings
\`\`\`bash
npm run codegen
\`\`\`
Generates \`src/generated/events.ts\` and \`src/generated/schema.ts\`.

### 5. Write your handler logic
Edit \`src/mapping.ts\` to implement \`handle_event\` and your per-event functions.

### 6. Compile to WASM
\`\`\`bash
npm run build
\`\`\`
Outputs \`build/mapping.wasm\`.

### 7. Deploy
Use the Logrix Helm chart with your \`logrix.yaml\` and \`build/mapping.wasm\`.
Refer to the Logrix deployment documentation for Helm values.
`;
}
// Standard ERC-20 ABI (events only) used as a starter placeholder.
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
