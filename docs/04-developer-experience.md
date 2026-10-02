# Developer Experience & Local "Run on the Go" Workflow

Logrix is engineered so that a developer can go from discovering the project to querying live blockchain data in under **60 seconds**.

---

## 1. Quick Installation

```bash
# macOS / Linux via Homebrew
brew install logrix/tap/logrix

# Direct shell installer (Linux / macOS)
curl -fsSL https://get.logrix.dev | sh

# Cargo install (for Rust developers)
cargo install logrix-cli
```

---

## 2. Interactive Scaffolding: `logrix init`

Run the initialization wizard to create a new indexer:

```bash
logrix init my-uniswap-indexer
```

The wizard prompts for:
1. **Target Chain:** Ethereum, Base, Arbitrum, Polygon, Optimism, or Custom RPC.
2. **Contract Address(es):** e.g., `0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640` (Uniswap v3 USDC/ETH pool).
3. **ABI Source:** Auto-fetch from Etherscan / Blockscout, or path to local ABI JSON.
4. **Events to Index:** e.g., `Swap`, `Mint`, `Burn`.
5. **Logic Style:** Declarative (YAML) or TypeScript / WASM.

This generates a clean project layout:

```text
my-uniswap-indexer/
├── logrix.yaml        # Main indexer configuration
├── schema.yaml        # Database tables & GraphQL schema definitions
├── abis/
│   └── Pool.json      # Contract ABI
└── handlers/          # (Optional) TypeScript or WASM logic
```

---

## 3. Local Development: `logrix dev`

To run the entire indexing stack locally with zero external dependencies:

```bash
logrix dev
```

What `logrix dev` does under the hood:
- Auto-detects Docker daemon and spins up default background containers for **PostgreSQL** (`logrix-postgres`) and **RabbitMQ** (`logrix-rabbitmq`) with healthchecks (or connects to custom ones if `DATABASE_URL` / `AMQP_URL` are set).
- Applies internal system migrations (checkpoints, DLQ tables, block state).
- Uses built-in chain presets for block times and safe confirmations.
- Spawns the Listener, Decoder, and dynamic GraphQL API on `http://localhost:4000/graphql`.
- Watches `logrix.yaml` and `schema.yaml` for instant hot-reloading with zero manual configuration.

---

## 4. Writing Indexing Logic

### Level 1: Declarative YAML (Zero Code)
Ideal for standard transfers, swaps, mints, and event logs:

```yaml
# logrix.yaml
handlers:
  - event: Pool.Swap
    mode: declarative
    table: swaps
    map:
      pool: log.address
      sender: args.sender
      recipient: args.recipient
      amount0: args.amount0
      amount1: args.amount1
      block_number: log.block_number
      tx_hash: log.tx_hash
```

Corresponding table schema:

```yaml
# schema.yaml
tables:
  swaps:
    columns:
      id: { type: string, primary_key: true } # auto-generated (chain_hash_logIndex)
      pool: { type: string, index: true }
      sender: { type: string, index: true }
      recipient: { type: string }
      amount0: { type: numeric }
      amount1: { type: numeric }
      block_number: { type: bigint, index: true }
      tx_hash: { type: string }
```

### Level 2: TypeScript / WASM Logic
When you need to compute running balances, calculate volume totals, or perform arithmetic across multiple events:

```typescript
// handlers/swap.ts
import { EventHandlerContext } from "@logrix/indexer";

export async function onSwap(ctx: EventHandlerContext) {
  const { event, db } = ctx;

  // Read existing pool state
  let pool = await db.get("pools", event.address);
  if (!pool) {
    pool = { id: event.address, totalVolumeUSD: 0, swapCount: 0 };
  }

  // Update aggregated metrics
  pool.swapCount += 1;
  await db.set("pools", pool);

  // Insert individual swap record
  await db.insert("swaps", {
    id: `${event.transactionHash}-${event.logIndex}`,
    pool: event.address,
    amount0: event.args.amount0,
    amount1: event.args.amount1,
  });
}
```

Logrix compiles this TypeScript code into an ultra-fast WebAssembly module executing securely inside `wasmtime`.

---

## 5. Instant GraphQL API

As soon as `logrix dev` starts, you can query your data via GraphQL:

```graphql
query GetRecentSwaps {
  swaps(
    first: 10
    orderBy: block_number
    orderDirection: desc
    where: { pool: "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640" }
  ) {
    id
    sender
    recipient
    amount0
    amount1
    block_number
  }
}
```

Includes automatic pagination (`first`, `skip`, `after`), filtering (`where`), and sorting (`orderBy`).
