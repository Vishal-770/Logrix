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
```

---

## CLI Commands

### `logrix init [name]`

Scaffolds a new indexer project. Run interactively or pass flags for CI/CD automation.

Options:
- `-y, --yes` — non-interactive mode using sensible defaults
- `--network <name>` — network preset (`arbitrum-one`, `ethereum`, `base`, `polygon`, `sepolia`)
- `--rpc <url>` — custom RPC endpoint
- `--contract-name <name>` — contract name
- `--address <address>` — target contract 0x address
- `--start-block <block>` — initial indexing block

Creates:
```
my-indexer/
  logrix.yaml          # indexer manifest: network, contracts, ABIs, WASM path
  schema.graphql       # entity definitions
  abis/
    TokenContract.json # contract ABI
  src/
    mapping.ts         # handler logic
  package.json
  tsconfig.json
  .gitignore
  README.md
```

---

### `logrix add contract <name>`

Adds an additional smart contract to an existing `logrix.yaml` project for multi-contract indexing.

```bash
npx logrix add contract SecondaryToken \
  --address 0x1111111111111111111111111111111111111111 \
  --abi ./my-abis/SecondaryToken.json \
  --start-block 1000000
```

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
- `src/generated/events.ts` — typed event and parameter classes (namespaced per contract)
- `src/generated/schema.ts` — entity classes with static `.load(id)`, `.remove(id)`, and `.save()`

---

### `logrix build`

Compiles your AssemblyScript handler into an optimized WebAssembly module (`build/mapping.wasm`).

```bash
npx logrix build
```

Options:
- `--entry <file>` — handler entry point (default: `src/mapping.ts`)
- `--out <file>` — output WASM binary path (default: `build/mapping.wasm`)
- `--debug` — compile without optimizations for debugging

---

## Runtime API

Imported via `import { ... } from "@logrix/sdk"` in AssemblyScript handlers:

| Export | Description |
|--------|-------------|
| `store.get(entity, id)` | Load raw entity JSON string from persistent store |
| `store.set(entity, id, data)` | Write entity to persistent store and stage for DB |
| `store.remove(entity, id)` | Mark entity as deleted and stage removal |
| `Entity.load(id)` | Auto-generated on each `@entity` class; loads existing entity or returns `null` |
| `Entity.remove(id)` | Auto-generated on each `@entity` class; removes entity by ID |
| `entity.save()` | Saves and persists entity state |
| `BigInt` | EVM 256-bit safe arithmetic: `.plus()`, `.minus()`, `.times()`, `.div()`, `.equals()`, `.gt()`, `.lt()` |
| `Address` | Address wrapper: `.fromString()`, `.toHexString()`, `.equals()` |
| `Bytes` | Hex byte wrapper: `.fromHexString()`, `.toHexString()`, `.length` |
| `log` | Structured logging: `log.info()`, `log.warning()`, `log.error()`, `log.debug()` |
| `allocate(size)` | Guest memory allocator (must be re-exported by `mapping.ts`) |

---

## Writing Handlers (`src/mapping.ts`)

```typescript
import { EventLog, log, BigInt, allocate } from "@logrix/sdk";
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

  entity.fromAddress = event.params.from;
  entity.toAddress = event.params.to;
  entity.amount = event.params.value; // Typed BigInt
  entity.save();

  log.info("Indexed transfer: " + entityId);
  return 0;
}
```

---

## Deployment

Deploy your compiled indexer on AWS or local Kubernetes:
- `logrix.yaml` contains all parameters for the Helm deployment
- `build/mapping.wasm` provides the compiled logic

Refer to the [Logrix Helm Chart](https://github.com/Vishal-770/Logrix) for automated deployment instructions.
