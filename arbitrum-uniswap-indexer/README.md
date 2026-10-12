# arbitrum-uniswap-indexer

Blockchain indexer built with [Logrix](https://github.com/Vishal-770/Logrix).

## Development Workflow

### 1. Install dependencies
```bash
npm install
```

### 2. Configure `logrix.yaml`
Set your chain, contract addresses, and event handlers.
To add additional contracts:
```bash
npx logrix add contract AnotherContract --address 0x... --start-block 0
```

### 3. Generate typed bindings
```bash
npm run codegen
```
Generates `src/generated/events.ts` and `src/generated/schema.ts`.

### 4. Validate configuration
```bash
npm run validate
```

### 5. Write handler logic
Implement your event indexing logic in `src/mapping.ts`.

### 6. Compile WebAssembly handler
```bash
npm run build
```
Produces `build/mapping.wasm`.

### 7. Export Helm deployment bundle
```bash
npm run export-values
```
Produces `indexer-values.yaml` bundling your WASM binary, GraphQL schema, and contract manifest.

---

## Deployment (Helm)

### Mode 1: Self-Hosted Deployment (Default)
Deploys out-of-the-box with built-in PostgreSQL and RabbitMQ pods. No extra infrastructure config needed:
```bash
helm install arbitrum-uniswap-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f indexer-values.yaml
```
*(Optional S3 cold block archiving can be enabled with `--set builtIn.minio=true`)*

*(To use an existing database or queue securely, configure credentials in `deploy/secrets.example.yaml`, run `kubectl apply -f deploy/secrets.example.yaml`, and pass `--set config.existingSecret=logrix-secrets`)*

Port-forward and open the GraphQL playground:
```bash
kubectl port-forward svc/arbitrum-uniswap-indexer-logrix-api 4000:4000
```
Visit **http://localhost:4000/** in your browser.

### Mode 2: Full AWS Production (EKS + Terraform)
Deploys with Aurora Serverless PostgreSQL, Amazon SQS, and KEDA autoscaling:
1. *(Optional)* If provisioning AWS infrastructure from scratch, clone https://github.com/Vishal-770/Logrix and run `deploy/terraform/aws`.
2. Configure credentials in `deploy/secrets.example.yaml` and apply:
   ```bash
   kubectl apply -f deploy/secrets.example.yaml
   ```
3. Deploy Helm release:
   ```bash
   helm install arbitrum-uniswap-indexer oci://ghcr.io/vishal-770/charts/logrix \
     -f deploy/values-aws.yaml \
     -f indexer-values.yaml \
     --set config.existingSecret=logrix-secrets
   ```
