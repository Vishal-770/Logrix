"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.LogrixConfigSchema = exports.WebhookSchema = exports.DataSourceSchema = exports.DataSourceMappingSchema = exports.AbiRefSchema = exports.EventHandlerSchema = void 0;
const zod_1 = require("zod");
exports.EventHandlerSchema = zod_1.z.object({
    event: zod_1.z.string().min(1, "Event signature is required"),
    handler: zod_1.z.string().min(1, "Handler function name is required"),
});
exports.AbiRefSchema = zod_1.z.object({
    name: zod_1.z.string().min(1, "ABI name is required"),
    file: zod_1.z.string().min(1, "ABI file path is required"),
});
exports.DataSourceMappingSchema = zod_1.z.object({
    kind: zod_1.z.string().default("wasm/assemblyscript"),
    source: zod_1.z.string().optional(),
    file: zod_1.z.string().default("./build/mapping.wasm"),
    fuelLimit: zod_1.z.number().int().positive().optional(),
    maxMemoryMb: zod_1.z.number().int().positive().optional(),
    enabled: zod_1.z.boolean().optional(),
    abis: zod_1.z.array(exports.AbiRefSchema).min(1, "At least one ABI reference is required"),
    eventHandlers: zod_1.z.array(exports.EventHandlerSchema).min(1, "At least one event handler is required"),
});
exports.DataSourceSchema = zod_1.z.object({
    kind: zod_1.z.string().default("ethereum/contract"),
    name: zod_1.z.string().regex(/^[A-Za-z][A-Za-z0-9_]*$/, "DataSource name must be alphanumeric"),
    network: zod_1.z.string().min(1, "Network name is required"),
    source: zod_1.z.object({
        address: zod_1.z.string().regex(/^0x[0-9a-fA-F]{40}$/, "Invalid 0x hex EVM contract address"),
        abi: zod_1.z.string().min(1, "ABI reference is required"),
        startBlock: zod_1.z.number().int().nonnegative().default(0),
    }),
    mapping: exports.DataSourceMappingSchema,
});
exports.WebhookSchema = zod_1.z.object({
    url: zod_1.z.string().url("Webhook destination must be a valid URL"),
    events: zod_1.z.array(zod_1.z.string()).default(["reorg"]),
    maxRetries: zod_1.z.number().int().nonnegative().default(3),
    secret: zod_1.z.string().optional(),
});
exports.LogrixConfigSchema = zod_1.z.object({
    specVersion: zod_1.z.string().default("0.1.0"),
    network: zod_1.z.object({
        name: zod_1.z.string().min(1, "Network name is required"),
        chainId: zod_1.z.number().int().positive("Chain ID must be positive"),
        rpcUrl: zod_1.z.string().url("Primary RPC URL must be a valid URL"),
        fallbackRpcUrls: zod_1.z.array(zod_1.z.string()).optional(),
    }),
    dataSources: zod_1.z.array(exports.DataSourceSchema).min(1, "At least one dataSource is required"),
    webhooks: zod_1.z.array(exports.WebhookSchema).optional(),
    indexing: zod_1.z.object({
        ringBufferDepth: zod_1.z.number().int().positive().default(128),
        reconcilerIntervalSecs: zod_1.z.number().int().positive().default(30),
        chunkSize: zod_1.z.number().int().positive().optional(),
        reconcilerChunkSize: zod_1.z.number().int().positive().optional(),
        reconcilerLagThreshold: zod_1.z.number().int().nonnegative().optional(),
        chainHeadPollMs: zod_1.z.number().int().positive().optional(),
        maxQueueDepth: zod_1.z.number().int().positive().optional(),
        rpcTimeoutSecs: zod_1.z.number().int().positive().optional(),
        rpcCbThreshold: zod_1.z.number().int().positive().optional(),
        rpcCooldownSecs: zod_1.z.number().int().positive().optional(),
        cuBudget: zod_1.z.number().int().positive().optional(),
        bulkStreamUrl: zod_1.z.string().optional(),
        databaseMaxConnections: zod_1.z.number().int().positive().optional(),
        databaseMinConnections: zod_1.z.number().int().positive().optional(),
        rabbitmqPrefetch: zod_1.z.number().int().positive().optional(),
        rabbitmqTimeoutSecs: zod_1.z.number().int().positive().optional(),
        sqsWaitTimeSeconds: zod_1.z.number().int().min(0).max(20).optional(),
        sqsMaxMessages: zod_1.z.number().int().min(1).max(10).optional(),
        wasmFuelLimit: zod_1.z.number().int().positive().optional(),
        wasmMaxMemoryMb: zod_1.z.number().int().positive().optional(),
        graphqlMaxDepth: zod_1.z.number().int().positive().optional(),
        graphqlMaxComplexity: zod_1.z.number().int().positive().optional(),
        graphqlDefaultLimit: zod_1.z.number().int().positive().optional(),
        graphqlMaxLimit: zod_1.z.number().int().positive().optional(),
        graphqlQueryTimeoutSecs: zod_1.z.number().int().positive().optional(),
        webhookTimeoutSecs: zod_1.z.number().int().positive().optional(),
        webhookMaxRetries: zod_1.z.number().int().nonnegative().optional(),
    }).passthrough().optional(),
});
