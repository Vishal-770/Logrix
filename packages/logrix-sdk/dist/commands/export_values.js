"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.exportValuesCommand = exportValuesCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const picocolors_1 = __importDefault(require("picocolors"));
const yaml_1 = require("yaml");
const templates_1 = require("../templates");
const ui_1 = require("../ui");
const errors_1 = require("../errors");
/**
 * logrix export-values
 *
 * Reads logrix.yaml, schema.graphql, and build/mapping.wasm then writes an
 * indexer-values.yaml that is ready to be passed to:
 *   helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
 *     -f indexer-values.yaml
 */
function exportValuesCommand() {
    const cmd = new commander_1.Command("export-values");
    cmd
        .description("Export a Helm-compatible indexer-values.yaml from this project")
        .option("--out <file>", "Output file path", "indexer-values.yaml")
        .option("--chain-id <id>", "Override chain ID (default: from logrix.yaml)")
        .option("--rpc <url>", "Override RPC URL (default: from logrix.yaml)")
        .option("--deploy-profiles", "Regenerate deploy/ profiles (values-local.yaml, values-aws.yaml, secrets.example.yaml)")
        .action((opts) => {
        ui_1.ui.banner("Export Helm Values", "Generating Kubernetes deployment values");
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");
        if (!fs.existsSync(configPath)) {
            throw new errors_1.CliError("logrix.yaml not found", "Project manifest is missing in current working directory", "Run this command from your indexer project root directory");
        }
        let config;
        try {
            config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
        }
        catch (e) {
            throw new errors_1.CliError("Failed to parse logrix.yaml", e.message);
        }
        // Helper to resolve ${ENV_VAR} placeholders
        const resolveEnv = (val, fieldName) => {
            if (!val || typeof val !== "string")
                return val;
            return val.replace(/\$\{([A-Z0-9_]+)\}/g, (_, envVar) => {
                const resolved = process.env[envVar];
                if (!resolved) {
                    ui_1.ui.error(`Unresolved environment placeholder in ${fieldName}`, `Found placeholder \${${envVar}} but environment variable ${envVar} is not set.`, `Export ${envVar}=<value> in your shell or .env file before running export-values.`);
                    process.exit(1);
                }
                return resolved;
            });
        };
        // Resolve network settings
        const chainId = opts.chainId
            ? parseInt(opts.chainId, 10)
            : config.network?.chainId ?? 1;
        const rawRpcUrl = opts.rpc ?? config.network?.rpcUrl ?? "";
        const rpcUrl = resolveEnv(rawRpcUrl, "network.rpcUrl");
        if (!rpcUrl) {
            ui_1.ui.error("RPC URL not configured", "network.rpcUrl is missing in logrix.yaml and --rpc was not passed.", "Specify --rpc <url> or add network.rpcUrl to logrix.yaml");
            process.exit(1);
        }
        // Build contracts manifest array (Rust engine format)
        const contracts = [];
        let minStartBlock = Number.MAX_SAFE_INTEGER;
        for (const ds of config.dataSources ?? []) {
            const events = (ds.mapping?.eventHandlers ?? []).map((h) => h.event);
            const startBlock = ds.source?.startBlock ?? 0;
            if (startBlock < minStartBlock) {
                minStartBlock = startBlock;
            }
            const contractEntry = {
                name: ds.name,
                address: ds.source?.address ?? "",
                start_block: startBlock,
                events,
                wasm_handler: "/etc/logrix/handlers/mapping.wasm",
            };
            if (typeof ds.mapping?.enabled === "boolean") {
                contractEntry.enabled = ds.mapping.enabled;
            }
            if (typeof ds.mapping?.fuelLimit === "number") {
                contractEntry.fuel_limit = ds.mapping.fuelLimit;
            }
            if (typeof ds.mapping?.maxMemoryMb === "number") {
                contractEntry.max_memory_mb = ds.mapping.maxMemoryMb;
            }
            contracts.push(contractEntry);
        }
        if (minStartBlock === Number.MAX_SAFE_INTEGER) {
            minStartBlock = 0;
        }
        // Build webhooks manifest array (Rust engine format)
        const webhooks = [];
        for (const wh of config.webhooks ?? []) {
            if (!wh.url)
                continue;
            const entry = {
                url: resolveEnv(wh.url, "webhooks[].url"),
                events: Array.isArray(wh.events) ? wh.events : ["reorg"],
                max_retries: wh.maxRetries ?? wh.max_retries ?? 3,
            };
            if (wh.secret) {
                entry.secret = resolveEnv(wh.secret, "webhooks[].secret");
            }
            webhooks.push(entry);
        }
        const manifest = {
            schema_version: "1",
            chain_id: chainId,
            contracts,
        };
        if (webhooks.length > 0) {
            manifest.webhooks = webhooks;
        }
        // Read schema.graphql
        const schemaPath = path.join(projectDir, "schema.graphql");
        if (!fs.existsSync(schemaPath)) {
            ui_1.ui.error("schema.graphql not found", "GraphQL entity schema is missing.", "Ensure schema.graphql exists in your project root.");
            process.exit(1);
        }
        const schemaContent = fs.readFileSync(schemaPath, "utf-8");
        // Read compiled WASM (base64)
        const wasmPath = path.join(projectDir, "build", "mapping.wasm");
        if (!fs.existsSync(wasmPath)) {
            ui_1.ui.error("Compiled WASM not found", "build/mapping.wasm does not exist.", "Run `logrix build` before exporting Helm values.");
            process.exit(1);
        }
        const wasmBase64 = fs.readFileSync(wasmPath).toString("base64");
        // Resolve advanced network and engine settings
        const fallbackUrls = config.network?.fallbackRpcUrls ?? [];
        const rpcFallbackStr = Array.isArray(fallbackUrls) ? fallbackUrls.join(",") : "";
        const reconcilerIntervalSecs = config.indexing?.reconcilerIntervalSecs ?? 30;
        const ringBufferDepth = config.indexing?.ringBufferDepth ?? 128;
        // Build indexer-values.yaml
        const values = {
            config: {
                chainId,
                rpcUrl,
                rpcFallbackUrls: rpcFallbackStr,
                contractAddress: contracts[0]?.address ?? "",
                startBlock: String(minStartBlock),
                reconcilerIntervalSecs,
                ringBufferDepth,
                manifestContent: (0, yaml_1.stringify)(manifest, { indent: 2 }),
                schemaContent,
                wasmBase64,
            },
        };
        const idx = config.indexing ?? {};
        if (idx.chunkSize !== undefined)
            values.config.chunkSize = idx.chunkSize;
        if (idx.reconcilerChunkSize !== undefined)
            values.config.reconcilerChunkSize = idx.reconcilerChunkSize;
        if (idx.reconcilerLagThreshold !== undefined)
            values.config.reconcilerLagThreshold = idx.reconcilerLagThreshold;
        if (idx.chainHeadPollMs !== undefined)
            values.config.chainHeadPollMs = idx.chainHeadPollMs;
        if (idx.maxQueueDepth !== undefined)
            values.config.maxQueueDepth = idx.maxQueueDepth;
        if (idx.rpcTimeoutSecs !== undefined)
            values.config.rpcTimeoutSecs = idx.rpcTimeoutSecs;
        if (idx.rpcCbThreshold !== undefined)
            values.config.rpcCbThreshold = idx.rpcCbThreshold;
        if (idx.rpcCooldownSecs !== undefined)
            values.config.rpcCooldownSecs = idx.rpcCooldownSecs;
        if (idx.cuBudget !== undefined)
            values.config.cuBudget = idx.cuBudget;
        if (idx.bulkStreamUrl !== undefined)
            values.config.bulkStreamUrl = idx.bulkStreamUrl;
        if (idx.databaseMaxConnections !== undefined)
            values.config.databaseMaxConnections = idx.databaseMaxConnections;
        if (idx.databaseMinConnections !== undefined)
            values.config.databaseMinConnections = idx.databaseMinConnections;
        if (idx.rabbitmqPrefetch !== undefined)
            values.config.rabbitmqPrefetch = idx.rabbitmqPrefetch;
        if (idx.rabbitmqTimeoutSecs !== undefined)
            values.config.rabbitmqTimeoutSecs = idx.rabbitmqTimeoutSecs;
        if (idx.sqsWaitTimeSeconds !== undefined)
            values.config.sqsWaitTimeSeconds = idx.sqsWaitTimeSeconds;
        if (idx.sqsMaxMessages !== undefined)
            values.config.sqsMaxMessages = idx.sqsMaxMessages;
        if (idx.wasmFuelLimit !== undefined)
            values.config.wasmFuelLimit = idx.wasmFuelLimit;
        if (idx.wasmMaxMemoryMb !== undefined)
            values.config.wasmMaxMemoryMb = idx.wasmMaxMemoryMb;
        if (idx.graphqlMaxDepth !== undefined)
            values.config.graphqlMaxDepth = idx.graphqlMaxDepth;
        if (idx.graphqlMaxComplexity !== undefined)
            values.config.graphqlMaxComplexity = idx.graphqlMaxComplexity;
        if (idx.graphqlDefaultLimit !== undefined)
            values.config.graphqlDefaultLimit = idx.graphqlDefaultLimit;
        if (idx.graphqlMaxLimit !== undefined)
            values.config.graphqlMaxLimit = idx.graphqlMaxLimit;
        if (idx.graphqlQueryTimeoutSecs !== undefined)
            values.config.graphqlQueryTimeoutSecs = idx.graphqlQueryTimeoutSecs;
        if (idx.webhookTimeoutSecs !== undefined)
            values.config.webhookTimeoutSecs = idx.webhookTimeoutSecs;
        if (idx.webhookMaxRetries !== undefined)
            values.config.webhookMaxRetries = idx.webhookMaxRetries;
        if (webhooks.length > 0) {
            values.config.webhookUrl = webhooks[0].url;
            values.webhook = {
                enabled: true,
            };
        }
        const outPath = path.resolve(projectDir, opts.out);
        fs.writeFileSync(outPath, (0, yaml_1.stringify)(values, { indent: 2 }));
        // Ensure deployment profile directory exists
        const deployDir = path.join(projectDir, "deploy");
        if (!fs.existsSync(deployDir) || opts.deployProfiles) {
            fs.mkdirSync(deployDir, { recursive: true });
            const answers = {
                projectName: path.basename(projectDir),
                networkName: config.network?.name ?? "network",
                chainId,
                rpcUrl,
                fallbackRpcUrls: Array.isArray(fallbackUrls) ? fallbackUrls : [],
                contractName: contracts[0]?.name ?? "Contract",
                contractAddress: contracts[0]?.address ?? "",
                startBlock: minStartBlock,
                webhookUrl: webhooks[0]?.url,
                ringBufferDepth,
                reconcilerIntervalSecs,
            };
            const awsPath = path.join(deployDir, "values-aws.yaml");
            if (!fs.existsSync(awsPath) || opts.deployProfiles) {
                fs.writeFileSync(awsPath, (0, templates_1.valuesAwsTemplate)(answers));
            }
            const secretPath = path.join(deployDir, "secrets.example.yaml");
            if (!fs.existsSync(secretPath) || opts.deployProfiles) {
                fs.writeFileSync(secretPath, (0, templates_1.secretsExampleTemplate)(answers));
            }
        }
        ui_1.ui.success("Helm values generated successfully", opts.out);
        ui_1.ui.label("Contracts", String(contracts.length));
        ui_1.ui.label("Start Block", String(minStartBlock));
        ui_1.ui.label("Chain ID", String(chainId));
        ui_1.ui.label("Webhooks", String(webhooks.length));
        ui_1.ui.label("WASM Handler Size", `${(Buffer.from(wasmBase64, "base64").length / 1024).toFixed(1)} KB`);
        ui_1.ui.section("Deployment Options");
        console.log(`  ${picocolors_1.default.bold("Mode 1: Self-Hosted (Default In-Cluster)")}`);
        console.log(`    helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f ${opts.out}`);
        console.log();
        console.log(`  ${picocolors_1.default.bold("Mode 2: Production AWS EKS (Aurora + SQS + IAM IRSA)")}`);
        console.log(`    helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-aws.yaml -f ${opts.out} --set config.existingSecret=logrix-secrets`);
        console.log();
    });
    return cmd;
}
