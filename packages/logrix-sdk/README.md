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

## CLI Commands

### `logrix init [name]`

Scaffolds a new indexer project with built-in presets for 26+ popular mainnets and testnets. Run interactively or pass flags for CI/CD automation.

Options:
- `-y, --yes` -- non-interactive mode using sensible defaults
- `--network <name>` -- network preset (`arbitrum-one`, `ethereum`, `base`, `optimism`, `polygon`, `bsc`, `avalanche`, `sepolia`, etc.)
- `--rpc <url>` -- custom RPC endpoint
- `--contract-name <name>` -- contract name
- `--address <address>` -- target contract 0x address
- `--start-block <block>` -- initial indexing block

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

### `logrix status`

Checks live JSON-RPC connectivity, measures round-trip latency, verifies chain ID match, checks latest block height, and verifies that target smart contracts are deployed with on-chain bytecode.

```bash
npx logrix status
```

Output includes:
- Network Name & Chain ID verification
- RPC reachability and response latency (ms)
- Latest chain block number
- Contract deployment verification (`eth_getCode` bytecode inspection)
- Sync scope and remaining block delta

---

### `logrix add contract <name>`

Adds an additional smart contract to an existing `logrix.yaml` project for multi-contract indexing.

```bash
# Add with manual ABI path
npx logrix add contract SecondaryToken \
  --address 0x1111111111111111111111111111111111111111 \
  --abi ./my-abis/SecondaryToken.json \
  --start-block 1000000

# Add with automatic ABI fetch from block explorers / Sourcify
npx logrix add contract DaiToken \
  --address 0x6b175474e89094c44da98b954eedeac495271d0f \
  --fetch-abi
```

Options:
- `--address <address>` -- contract address
- `--abi <path>` -- path to local ABI JSON
- `--fetch-abi` -- auto-fetch verified contract ABI from block explorers (Etherscan, Arbiscan, Basescan, Polygonscan, etc.) and Sourcify
- `--etherscan-key <key>` -- optional block explorer API key for rate limits
- `--start-block <block>` -- initial indexing block

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

Reads `logrix.yaml`, `schema.graphql`, and `build/mapping.wasm` to produce a ready-to-deploy `indexer-values.yaml` for Helm.

```bash
npx logrix export-values
```

Options:
- `--out <file>` -- output values file (default: `indexer-values.yaml`)
- `--chain-id <id>` -- override chain ID
- `--rpc <url>` -- override RPC URL

Deploy directly to Kubernetes:
```bash
helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
  -f cluster-infra.yaml \
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

1. Use `@logrix/sdk` to build and export your indexer:
   ```bash
   logrix codegen && logrix build && logrix export-values
   ```
2. Deploy via Helm to your Kubernetes cluster:
   ```bash
   helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
     -f cluster-infra.yaml \
     -f indexer-values.yaml
   ```

Refer to the [Logrix Helm Chart](https://github.com/Vishal-770/Logrix) for full Helm deployment options.
