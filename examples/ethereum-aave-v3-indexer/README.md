# ethereum-aave-v3-indexer

Production-grade Aave V3 Core Lending Indexer on Ethereum Mainnet built with `@logrix/sdk` 0.8.1.

Tracks the full lifecycle of lending and borrowing on Ethereum Mainnet:
- **Supply & Withdraw**: Collateral deposits and liquidity withdrawals.
- **Borrow & Repay**: Debt generation, variable borrow rates, and repayments.
- **LiquidationCall**: Collateral auctions, bad-debt coverage, and liquidator bonuses.
- **FlashLoan**: Uncollateralized flashloans, volumes, and protocol premiums.

---

## Contract Target

| Property | Value |
| :--- | :--- |
| **Network** | Ethereum Mainnet (`chainId: 1`) |
| **Aave V3 Pool** | `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` |
| **Deployment Block** | `16291127` |
| **Language** | AssemblyScript (`asc`) compiled to WebAssembly |

---

## Developer Workflow

### 1. Install Dependencies
```bash
npm install
```

### 2. Generate AssemblyScript Bindings
Generates typed event classes and GraphQL entity accessors from `abis/Pool.json` and `schema.graphql`:
```bash
npm run codegen
# or: npx logrix codegen
```

### 3. Validate Configuration
Checks YAML syntax, ABI completeness, contract address formatting, and handler paths:
```bash
npm run validate
# or: npx logrix validate
```

### 4. Compile WebAssembly Mapping
Compiles `src/mapping.ts` to optimized WebAssembly (`build/mapping.wasm`):
```bash
npm run build
# or: npx logrix build
```

---

## Production Deployment to AWS

Per Logrix deployment invariants, no source code is built on remote servers. Deployment uses pre-built, versioned Docker container images via Helm:

### 1. Export Helm Deployment Values
```bash
npx logrix export-values --deploy-profiles
```
This generates:
- `indexer-values.yaml`: Embedded WASM binary, GraphQL schema, and contract manifest.
- `deploy/values-aws.yaml`: Production AWS architecture profile (Aurora PostgreSQL, AWS SQS FIFO, S3 cold storage, IAM IRSA).

### 2. Deploy with Helm
```bash
helm install aave-v3-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/values-aws.yaml \
  -f indexer-values.yaml \
  --set config.existingSecret=logrix-secrets
```
