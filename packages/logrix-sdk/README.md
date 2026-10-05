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
npm run build
```

---

## CLI Commands

### `logrix init [name]`

Interactively scaffolds a new indexer project.

Prompts for:
- Project name
- Network (Ethereum, Arbitrum One, Base, Polygon, Sepolia, ...)
- RPC URL
- Contract name
- Contract address
- Start block

Creates:

```
my-indexer/
  logrix.yaml          # indexer config: chain, contract, events, WASM path
  schema.graphql       # entity definitions
  abis/
    MyContract.json    # placeholder ERC-20 ABI — replace with your contract ABI
  src/
    mapping.ts         # handler entry point (edit this)
  package.json
  tsconfig.json
  .gitignore
  README.md
```

---

### `logrix codegen`

Reads `logrix.yaml` + ABI files + `schema.graphql` and generates typed AssemblyScript bindings.

```bash
logrix codegen
```

Outputs:
- `src/generated/events.ts` — typed event classes per ABI event
- `src/generated/schema.ts` — entity classes with `.save()` methods

Re-run this every time you change your ABI or schema.

---

### `logrix build`

Compiles your AssemblyScript handler to WASM using `asc`.

```bash
logrix build
```

Options:
- `--entry <file>` — handler entry point (default: `src/mapping.ts`)
- `--out <file>` — output path (default: `build/mapping.wasm`)
- `--debug` — build without optimization

---

## Project Structure

### `logrix.yaml`

The main indexer configuration. Contains all values needed for deployment.

```yaml
specVersion: "0.1.0"

network:
  name: "arbitrum-one"
  chainId: 42161
  rpcUrl: "https://arb1.arbitrum.io/rpc"

dataSources:
  - kind: ethereum/contract
    name: "MyContract"
    network: "arbitrum-one"
    source:
      address: "0x..."
      abi: "MyContract"
      startBlock: 0
    mapping:
      kind: wasm/assemblyscript
      file: ./build/mapping.wasm
      abis:
        - name: "MyContract"
          file: ./abis/MyContract.json
      eventHandlers:
        - event: "Transfer(address indexed,address indexed,uint256)"
          handler: handleTransfer
```

### `schema.graphql`

Define entities using GraphQL SDL with `@entity`:

```graphql
type Transfer @entity {
  id: ID!
  blockNumber: BigInt!
  fromAddress: String! @index
  toAddress: String! @index
  amount: String!
  transactionHash: String!
  timestamp: BigInt!
}
```

### `src/mapping.ts`

Your handler logic. Must export `handle_event(ptr: i32, len: i32): i32` — this is the function the Logrix engine calls for every matched event log.

```typescript
import { allocate, EventLog } from "@logrix/sdk";
import { TransferEvent } from "./generated/events";
import { Transfer } from "./generated/schema";

export { allocate } from "@logrix/sdk";

export function handle_event(ptr: i32, len: i32): i32 {
  // decode the raw event JSON bytes from guest memory
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = load<u8>(ptr + i);
  }
  const raw = String.UTF8.decode(bytes.buffer);

  const log = new EventLog();
  const event = new TransferEvent(log);

  const entity = new Transfer(event.transactionHash + "-" + event.logIndex.toString());
  entity.fromAddress = event.params.from;
  entity.toAddress = event.params.to;
  entity.amount = event.params.value;
  entity.save();
  return 0;
}
```

---

## Runtime API

Imported via `import { ... } from "@logrix/sdk"` in AssemblyScript handlers:

| Export | Description |
|--------|-------------|
| `allocate(size: i32): i32` | Memory allocator — re-export this from your mapping |
| `logrix_emit(entity: string, json: string): void` | Emit an entity to the database |
| `logrix_db_get(key: string): string` | Read cross-block persisted state |
| `logrix_db_set(key: string, value: string): void` | Write cross-block persisted state |
| `logrix_log(level: i32, msg: string): void` | Log a message (0=DEBUG 1=INFO 2=WARN 3=ERROR) |
| `EventLog` | Raw event log class |

---

## Deployment

After `logrix build` succeeds, you have:
- `logrix.yaml` — all config values for your Helm deployment
- `build/mapping.wasm` — compiled handler

Use the [Logrix Helm chart](https://github.com/Vishal-770/Logrix) to deploy. Refer to the chart documentation for how to pass these files as Helm values.
