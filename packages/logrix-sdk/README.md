# @logrix/sdk

Official CLI and AssemblyScript runtime library for [Logrix](https://github.com/Vishal-770/Logrix) blockchain indexers.

[![npm](https://img.shields.io/npm/v/@logrix/sdk)](https://www.npmjs.com/package/@logrix/sdk)

---

## Quick Start

```bash
npx @logrix/sdk init my-indexer
cd my-indexer
npm install
npm run codegen
npm run validate
npm run build
npm test
```

---

## Manifest Configuration (`logrix.yaml`)

`logrix.yaml` is the single source of truth for your indexer project. Users do not need to author or edit Helm charts directly—all contract logic, event mappings, and operational tuning parameters are defined directly in `logrix.yaml`.

Running `npx @logrix/sdk export-values` automatically compiles `logrix.yaml` into a production-ready `indexer-values.yaml` bundle configured for deployment to the published Logrix Helm chart (`oci://ghcr.io/vishal-770/charts/logrix`).

### Complete `logrix.yaml` Example

```yaml
specVersion: 0.1.0

# 1. Target Blockchain Network
network:
  name: arbitrum-one
  chainId: 42161
  rpcUrl: "https://arb-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
  fallbackRpcUrls:
    - "https://arb1.arbitrum.io/rpc"

# 2. Smart Contracts & Mapping Handlers
dataSources:
  - kind: ethereum/contract
    name: UniswapV3Pool
    network: arbitrum-one
    source:
      address: "0xC6962004f452bE9203591991D15f6b388e09E8D0"
      abi: UniswapV3Pool
      startBlock: 514022000
    mapping:
      kind: wasm/assemblyscript
      file: ./build/mapping.wasm
      # Per-contract resource limits & toggles:
      enabled: true                  # Toggle to pause indexing without deleting config (default: true)
      fuelLimit: 5000000             # Instruction fuel budget override
      maxMemoryMb: 256               # WASM linear memory ceiling in MB
      abis:
        - name: UniswapV3Pool
          file: ./abis/UniswapV3Pool.json
      eventHandlers:
        - event: Swap(address indexed,address indexed,int256,int256,uint160,uint128,int24)
          handler: handleSwap
        - event: Mint(address,address indexed,int24 indexed,int24 indexed,uint128,uint256,uint256)
          handler: handleMint
        - event: Burn(address indexed,int24 indexed,int24 indexed,uint128,uint256,uint256)
          handler: handleBurn

# 3. Outbound Webhook Subscriptions
webhooks:
  - url: "https://api.mybackend.com/webhooks/reorgs"
    events: ["reorg"]
    maxRetries: 3
    secret: "${CRITICAL_WEBHOOK_SECRET}"

# 4. Operational & Engine Tuning (Optional - Sensible Defaults Applied)
indexing:
  # Ingestion & RPC Tuning
  chunkSize: 10                      # Maximum blocks queried per eth_getLogs batch (default: 10)
  chainHeadPollMs: 1500              # Polling interval at chain head in ms (default: 1500)
  maxQueueDepth: 1000                # Queue backpressure threshold before pausing (default: 1000)
  rpcTimeoutSecs: 10                 # RPC HTTP client timeout in seconds (default: 10)
  rpcCbThreshold: 5                  # Circuit breaker consecutive failure trip threshold (default: 5)
  rpcCooldownSecs: 30                # Circuit breaker cooldown in seconds (default: 30)

  # Reorg & Gap Reconciler Tuning
  ringBufferDepth: 128               # Rolling parent-hash ring buffer size (default: 128)
  reconcilerIntervalSecs: 30         # Periodic gap check interval in seconds (default: 30)
  reconcilerChunkSize: 10            # Chunk size when filling missing block ranges (default: 10)
  reconcilerLagThreshold: 50         # Blocks behind head before reconciling (default: 50)

  # Database Connection Limits
  databaseMaxConnections: 10         # Max PostgreSQL pool connections per pod (default: 10)
  databaseMinConnections: 2          # Min idle PostgreSQL pool connections (default: 2)

  # Queue Driver Parameters
  rabbitmqPrefetch: 100              # RabbitMQ consumer channel QoS prefetch (default: 100)
  rabbitmqTimeoutSecs: 10            # RabbitMQ connection timeout (default: 10)
  sqsWaitTimeSeconds: 2              # SQS receive message long polling wait (0-20s, default: 2)
  sqsMaxMessages: 1                  # SQS batch receive count (1-10, default: 1)

  # Webhook & GraphQL Guardrails
  webhookTimeoutSecs: 5              # Outgoing webhook HTTP request timeout (default: 5)
  webhookMaxRetries: 1               # Default delivery retry attempts (default: 1)
  graphqlMaxDepth: 7                 # GraphQL query max nesting depth (default: 7)
  graphqlMaxComplexity: 200          # GraphQL query complexity limit (default: 200)
  graphqlDefaultLimit: 100           # Default query result count (default: 100)
  graphqlMaxLimit: 1000              # Max query pagination size (default: 1000)
  graphqlQueryTimeoutSecs: 5         # Query execution timeout in seconds (default: 5)
```

---

## CLI Commands

### `logrix init [name]`

Scaffolds a new indexer project with built-in presets for 26+ popular mainnets and testnets. Run interactively or pass flags for CI/CD automation.

Options:
- `-y, --yes` -- non-interactive mode using sensible defaults
- `--network <name>` -- network preset (`arbitrum-one`, `ethereum`, `base`, `optimism`, `polygon`, `bsc`, `avalanche`, `sepolia`, etc.)
- `--rpc <url>` -- custom RPC endpoint
- `--fallback-rpc <urls...>` -- optional fallback RPC endpoints for RPC gateway failover
- `--contract-name <name>` -- contract name
- `--address <address>` -- target contract 0x address
- `--start-block <block>` -- initial indexing block
- `--webhook-url <url>` -- declarative webhook subscriber URL
- `--reorg-depth <depth>` -- reorg ring buffer depth (default: 128)
- `--reconciler-interval <secs>` -- gap reconciler interval in seconds (default: 30)

Creates:
```
my-indexer/
  logrix.yaml          # indexer manifest: network, contracts, ABIs, WASM path
  schema.graphql       # entity definitions
  abis/
    TokenContract.json # contract ABI
  src/
    mapping.ts         # handler logic
  deploy/
    values-local.yaml  # local Kubernetes profile (Minikube / Kind)
    values-aws.yaml    # production AWS profile (Aurora, SQS, S3, KEDA)
    secrets.example.yaml # Kubernetes Secret template for RPC/DB credentials
  package.json
  tsconfig.json
  .gitignore
  README.md
```

---

### `logrix status`

Checks live JSON-RPC connectivity across primary and fallback endpoints, measures round-trip latency, verifies chain ID match, checks latest block height, and verifies that target smart contracts are deployed with on-chain bytecode.

```bash
npx logrix status
```

Output includes:
- Network Name & Chain ID verification
- Primary and fallback RPC reachability and response latency (ms)
- Latest chain block number
- Contract deployment verification (`eth_getCode` bytecode inspection)
- Sync scope and remaining block delta
- Configured declarative webhooks

---

### `logrix add contract <name>` and `logrix add webhook <url>`

Adds an additional smart contract or declarative webhook to an existing `logrix.yaml` project.

```bash
# Add contract with manual ABI path
npx logrix add contract SecondaryToken \
  --address 0x1111111111111111111111111111111111111111 \
  --abi ./my-abis/SecondaryToken.json \
  --start-block 1000000

# Add contract with automatic ABI fetch from block explorers / Sourcify
npx logrix add contract DaiToken \
  --address 0x6b175474e89094c44da98b954eedeac495271d0f \
  --fetch-abi

# Add declarative webhook subscriber
npx logrix add webhook https://api.mybackend.com/webhooks/logrix \
  --events Transfer,reorg \
  --max-retries 3
```

Contract Options:
- `--address <address>` -- contract address
- `--abi <path>` -- path to local ABI JSON
- `--fetch-abi` -- auto-fetch verified contract ABI from block explorers and Sourcify
- `--etherscan-key <key>` -- optional block explorer API key for rate limits
- `--start-block <block>` -- initial indexing block

Webhook Options:
- `--events <events>` -- comma-separated event names (default: `reorg`)
- `--secret <secret>` -- HMAC signing secret (optional)
- `--max-retries <retries>` -- maximum delivery retries (default: 3)

---

### `logrix validate`

Validates your `logrix.yaml`, contract addresses, ABI files, `schema.graphql` entities, and handler mappings before compilation.

```bash
npx logrix validate
```

---

### `logrix codegen`

Parses `logrix.yaml` dataSources, ABI files, and `schema.graphql` to generate typed AssemblyScript bindings.

```bash
npx logrix codegen
```

Outputs:
- `src/generated/events.ts` -- typed event and parameter classes (namespaced per contract), multi-contract address constants, router helpers, and dynamic ABI decoding (`string`, `bytes`)
- `src/generated/schema.ts` -- entity classes with static `.load(id)`, `.remove(id)`, and `.save()`, handling `@derivedFrom` relationships

---

### `logrix build`

Compiles your AssemblyScript handler into an optimized WebAssembly module (`build/mapping.wasm`).

```bash
npx logrix build
```

Options:
- `--entry <file>` -- handler entry point (default: `src/mapping.ts`)
- `--out <file>` -- output WASM binary path (default: `build/mapping.wasm`)
- `--debug` -- compile without optimizations for debugging

---

### `logrix export-values`

Reads `logrix.yaml`, `schema.graphql`, and `build/mapping.wasm` to produce a ready-to-deploy `indexer-values.yaml` for Helm. Also ensures deployment profile templates exist in `deploy/`.

Environment placeholders such as `${ALCHEMY_API_KEY}` are preserved in generated values and manifests; they are never expanded into files. When placeholders are present, the exporter configures `config.existingSecret` and lists safe `CHANGE_ME` entries in `deploy/secrets.example.yaml`. Fill and apply that Secret before deploying, and do not commit real credentials.

```bash
npx logrix export-values
```

Options:
- `--out <file>` -- output values file (default: `indexer-values.yaml`)
- `--chain-id <id>` -- override chain ID
- `--rpc <url>` -- override RPC URL
- `--deploy-profiles` -- regenerate `deploy/` profile templates (`values-local.yaml`, `values-aws.yaml`, `secrets.example.yaml`)

Deploy with Helm:

**Local Kubernetes (Minikube / Kind):**
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/values-local.yaml \
  -f indexer-values.yaml
```

**Production AWS EKS:**
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/values-aws.yaml \
  -f indexer-values.yaml \
  --set config.existingSecret=logrix-aws-secrets
```

**Custom Kubernetes Cluster:**
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f my-cluster-infra.yaml \
  -f indexer-values.yaml
```

---

### `logrix test`

Runs local test validation across configuration, code generation, and WASM compilation, and executes any unit tests in `tests/`.

```bash
npx logrix test
```

---

## Runtime API

Imported via `import { ... } from "@logrix/sdk"` in AssemblyScript handlers:

| Export | Description |
|--------|-------------|
| `store.get(entity, id)` | Load raw entity JSON string from persistent store |
| `store.set(entity, id, data)` | Write entity to persistent store and stage for DB |
| `store.remove(entity, id)` | Mark entity as deleted and stage soft-delete |
| `Entity.load(id)` | Auto-generated on each `@entity` class; loads existing entity or returns `null` |
| `Entity.remove(id)` | Auto-generated on each `@entity` class; removes entity by ID |
| `entity.save()` | Saves and persists entity state |
| `BigInt` | EVM 256-bit safe arithmetic: `.plus()`, `.minus()`, `.times()`, `.div()`, `.equals()`, `.gt()`, `.lt()` |
| `Address` | Address wrapper: `.fromString()`, `.toHexString()`, `.equals()` |
| `Bytes` | Hex byte wrapper: `.fromHexString()`, `.fromUTF8()`, `.toHexString()`, `.toByteArray()`, `.length` |
| `crypto.keccak256(data)` | Computes standard EVM Keccak-256 hash |
| `formatUnits(val, decimals)` | Formats raw BigInt value into human-readable decimal string (default: 18) |
| `decodeDynamicString(data, idx)` | Decodes dynamic string parameter from raw ABI event data |
| `decodeDynamicBytes(data, idx)` | Decodes dynamic bytes parameter from raw ABI event data |
| `createMockEventLog(config)` | Creates mock EventLog instance for testing mapping logic |
| `log` | Structured logging: `log.info()`, `log.warning()`, `log.error()`, `log.debug()` |
| `allocate(size)` | Guest memory allocator (must be re-exported by `mapping.ts`) |

---

## Writing Handlers (`src/mapping.ts`)

```typescript
import { EventLog, log, BigInt, Address, crypto, formatUnits, allocate } from "@logrix/sdk";
import { TransferEvent } from "./generated/events";
import { Transfer } from "./generated/schema";

export { allocate } from "@logrix/sdk";

export function handle_event(ptr: i32, len: i32): i32 {
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = load<u8>(ptr + i);
  }
  const raw = String.UTF8.decode(bytes.buffer);

  if (raw.includes("Transfer")) {
    return handleTransfer(raw);
  }
  return 0;
}

function handleTransfer(raw: string): i32 {
  const event = new TransferEvent(new EventLog());

  const entityId = event.transactionHash + "-" + event.logIndex.toString();

  // Load existing entity or instantiate a new one
  let entity = Transfer.load(entityId);
  if (!entity) {
    entity = new Transfer(entityId);
  }

  entity.fromAddress = event.params.from.toHexString();
  entity.toAddress = event.params.to.toHexString();
  entity.amount = event.params.value; // Typed BigInt
  entity.save();

  log.info("Indexed transfer: " + entityId);
  return 0;
}
```

---

## Deployment

1. Use `@logrix/sdk` to build and export your indexer bundle:
   ```bash
   npm run codegen && npm run build && npm run export-values
   ```

2. Deploy using Helm:

   **Mode 1: Self-Hosted Deployment (Default)**
   Deploys out-of-the-box with built-in PostgreSQL and RabbitMQ pods. No extra infrastructure config needed:
   ```bash
   helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
     -f indexer-values.yaml
   ```
   *(Optional S3 block archiving can be enabled with `--set builtIn.minio=true`)*

   *(To connect your own external database or queue securely, configure `deploy/secrets.example.yaml`, apply with `kubectl apply -f deploy/secrets.example.yaml`, and pass `--set config.existingSecret=logrix-secrets`)*

   **Mode 2: Full AWS Production (EKS + Terraform)**
   Deploys with Aurora Serverless PostgreSQL, Amazon SQS, and KEDA autoscaling:
   ```bash
   helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
     -f deploy/values-aws.yaml \
     -f indexer-values.yaml \
     --set config.existingSecret=logrix-secrets
   ```

   **AWS Infrastructure Note:**
   The `@logrix/sdk` generates the application indexer bundle (`indexer-values.yaml`) and Helm deployment profile (`deploy/values-aws.yaml`). If you need to provision the underlying AWS infrastructure (VPC, EKS cluster, Aurora PostgreSQL, SQS queues, S3 bucket, IAM roles) from scratch, complete production Terraform modules are provided in the [Logrix GitHub Repository](https://github.com/Vishal-770/Logrix) under `deploy/terraform/aws/`.

Refer to the [Logrix Helm Chart](https://github.com/Vishal-770/Logrix) for full Helm deployment options.

---

## Release & Publishing

The `@logrix/sdk` package has an independent CI/CD release pipeline decoupled from container image builds.

To trigger an automated npm package release:
```bash
git tag sdk-v0.7.1
git push origin sdk-v0.7.1
```
The workflow runs test validation, compiles the TypeScript distribution, synchronizes version numbers, and publishes to the npm registry with provenance.
