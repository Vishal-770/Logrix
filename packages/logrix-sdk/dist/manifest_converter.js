"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.resolveEnv = resolveEnv;
exports.convertSdkManifestToRuntime = convertSdkManifestToRuntime;
exports.mapIndexingToHelmValues = mapIndexingToHelmValues;
const ui_1 = require("./ui");
/**
 * Resolves ${ENV_VAR} placeholders in string values.
 * Exits with clear error message if placeholder is unset.
 */
function resolveEnv(val, fieldName) {
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
}
/**
 * Converts high-level SDK project manifest to concrete Rust runtime engine format.
 */
function convertSdkManifestToRuntime(config, chainId) {
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
    return { contracts, webhooks, minStartBlock, manifest };
}
/**
 * Maps optional SDK indexing settings to Helm deployment values.
 */
function mapIndexingToHelmValues(values, idx = {}) {
    const mapping = [
        ["chunkSize", "chunkSize"],
        ["reconcilerChunkSize", "reconcilerChunkSize"],
        ["reconcilerLagThreshold", "reconcilerLagThreshold"],
        ["chainHeadPollMs", "chainHeadPollMs"],
        ["maxQueueDepth", "maxQueueDepth"],
        ["rpcTimeoutSecs", "rpcTimeoutSecs"],
        ["rpcCbThreshold", "rpcCbThreshold"],
        ["rpcCooldownSecs", "rpcCooldownSecs"],
        ["cuBudget", "cuBudget"],
        ["bulkStreamUrl", "bulkStreamUrl"],
        ["databaseMaxConnections", "databaseMaxConnections"],
        ["databaseMinConnections", "databaseMinConnections"],
        ["rabbitmqPrefetch", "rabbitmqPrefetch"],
        ["rabbitmqTimeoutSecs", "rabbitmqTimeoutSecs"],
        ["sqsWaitTimeSeconds", "sqsWaitTimeSeconds"],
        ["sqsMaxMessages", "sqsMaxMessages"],
        ["wasmFuelLimit", "wasmFuelLimit"],
        ["wasmMaxMemoryMb", "wasmMaxMemoryMb"],
        ["graphqlMaxDepth", "graphqlMaxDepth"],
        ["graphqlMaxComplexity", "graphqlMaxComplexity"],
        ["graphqlDefaultLimit", "graphqlDefaultLimit"],
        ["graphqlMaxLimit", "graphqlMaxLimit"],
        ["graphqlQueryTimeoutSecs", "graphqlQueryTimeoutSecs"],
        ["webhookTimeoutSecs", "webhookTimeoutSecs"],
        ["webhookMaxRetries", "webhookMaxRetries"],
    ];
    for (const [sdkKey, helmKey] of mapping) {
        if (idx[sdkKey] !== undefined) {
            values.config[helmKey] = idx[sdkKey];
        }
    }
}
