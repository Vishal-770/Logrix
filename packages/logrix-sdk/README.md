# logrix-sdk

> Official TypeScript & AssemblyScript SDK for the [Logrix](https://github.com/Vishal-770/Logrix) blockchain indexer.

Define custom, type-safe blockchain event mappings with sub-millisecond execution inside the Logrix WebAssembly sandbox.

---

## Installation

```bash
npm install logrix-sdk
npm install --save-dev assemblyscript
```

---

## Writing Event Handlers

Create your mapping logic in TypeScript (compiled to WebAssembly via AssemblyScript):

```typescript
import { EventLog, logrix_db_get, logrix_db_set, logrix_emit } from "logrix-sdk";

export function handleTransfer(event: EventLog): void {
  let sender = event.topics[1];
  let recipient = event.topics[2];
  let amount = event.data;

  // 1. Read existing state across blocks
  let currentBalance = logrix_db_get(sender);

  // 2. Perform custom calculation & update state
  let newBalance = updateBalance(currentBalance, amount);
  logrix_db_set(sender, newBalance);

  // 3. Emit structured entity for GraphQL queries
  logrix_emit("Transfer", JSON.stringify({
    id: `${event.transaction_hash}-${event.log_index}`,
    blockNumber: event.block_number,
    fromAddress: sender,
    toAddress: recipient,
    amount: amount,
    transactionHash: event.transaction_hash,
    timestamp: event.block_timestamp
  }));
}
```

---

## Compiling to WebAssembly

Compile your handlers to WebAssembly:

```bash
npx asc handlers/mapping.ts -o handlers/mapping.wasm --optimize --exportRuntime
```

---

## Deploying to Kubernetes

Once compiled, deploy to your Kubernetes cluster in one command:

```bash
# Local Kubernetes (Minikube / Kind / K3s)
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/helm/values-local.yaml \
  --set-file config.wasmBinary=handlers/mapping.wasm

# Production AWS EKS
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/helm/values-aws.yaml \
  --set-file config.wasmBinary=handlers/mapping.wasm
```

---

## API Reference

### `EventLog`
- `address: string` - Target contract address
- `block_number: u64` - Block height
- `block_hash: string` - Block hash
- `transaction_hash: string` - Transaction hash
- `log_index: u32` - Index of the log within the transaction
- `block_timestamp: u64` - Unix block timestamp
- `topics: Array<string>` - Indexed event topics (`topics[0]` is event signature)
- `data: string` - Unindexed ABI-encoded data payload

### Host Functions
- `logrix_emit(entityType: string, jsonPayload: string): void` - Persists entity to PostgreSQL and exposes via GraphQL.
- `logrix_db_get(key: string): string` - Reads key from persistent state store.
- `logrix_db_set(key: string, value: string): void` - Stores key-value pair across blocks.

---

## License

Apache-2.0
