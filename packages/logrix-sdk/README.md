# @logrix/sdk

Official TypeScript and AssemblyScript SDK for Logrix blockchain indexers.

Scaffold custom blockchain indexer projects, generate strongly typed event and GraphQL entity bindings, compile WebAssembly handlers, and deploy to Kubernetes with Helm.

---

## Quickstart

### 1. Scaffold a New Project
Initialize a complete project interactively:

```bash
npx @logrix/sdk init my-indexer
```

Follow the prompts to choose your network (Ethereum, Arbitrum, Base, Polygon), smart contract address, and starting block.

This scaffolds:
- `manifest.yaml`: Indexer configuration and target contract event subscriptions.
- `schema.graphql`: Entity definitions exposed via GraphQL.
- `abis/<ContractName>.json`: ABI definitions.
- `handlers/mapping.ts`: Strongly typed AssemblyScript event mapping logic.
- `values-local.yaml`: Ready-to-use Helm configuration for local Minikube / Kind / K3s clusters.
- `values-aws.yaml`: Ready-to-use Helm configuration for production AWS EKS clusters.

---

### 2. Code Generation

Generate strongly typed event classes and GraphQL entity models:

```bash
npm run codegen
```

Generated code in `handlers/generated/`:
- `events.ts`: Strongly typed event classes with `.params` parsed from ABI.
- `schema.ts`: Strongly typed entity classes with `.save()` method emitting entities to the Logrix engine.

---

### 3. Build WebAssembly Handlers

Compile your mapping logic into sandboxed WebAssembly:

```bash
npm run build
```

This generates `handlers/mapping.wasm`.

---

## Deploy to Kubernetes

Deploy your indexer manually with Helm using the scaffolded profiles:

### Local Kubernetes (Minikube / Kind / K3s)
Zero cloud infrastructure needed. Runs in-cluster PostgreSQL 16 and RabbitMQ:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f values-local.yaml \
  --set-file config.manifestContent=manifest.yaml \
  --set-file config.schemaContent=schema.graphql \
  --set-file config.wasmBinary=handlers/mapping.wasm
```

Port forward to open the GraphQL Playground:
```bash
kubectl port-forward svc/my-indexer-logrix-api 4000:4000
```
Open `http://localhost:4000/`.

### Production AWS EKS
Uses AWS Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f values-aws.yaml \
  --set-file config.manifestContent=manifest.yaml \
  --set-file config.schemaContent=schema.graphql \
  --set-file config.wasmBinary=handlers/mapping.wasm
```

---

## SDK API Reference

### Host Functions
- `logrix_emit(entityType: string, jsonPayload: string): void`: Persists entity to PostgreSQL and exposes via dynamic GraphQL query engine.
- `logrix_db_get(key: string): string`: Reads persisted key-value state across blocks.
- `logrix_db_set(key: string, value: string): void`: Writes key-value state across blocks.
- `logrix_log(level: i32, message: string): void`: Emits diagnostic log message (0=DEBUG, 1=INFO, 2=WARN, 3=ERROR).

### Data Structures
- `EventLog`: Raw blockchain event log payload with topics, data, block number, timestamp, and transaction hash.

---

## License

Apache-2.0
