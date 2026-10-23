import { z } from "zod";

export const EventHandlerSchema = z.object({
  event: z.string().min(1, "Event signature is required"),
  handler: z.string().min(1, "Handler function name is required"),
});

export const AbiRefSchema = z.object({
  name: z.string().min(1, "ABI name is required"),
  file: z.string().min(1, "ABI file path is required"),
});

export const DataSourceMappingSchema = z.object({
  kind: z.string().default("wasm/assemblyscript"),
  source: z.string().optional(),
  file: z.string().default("./build/mapping.wasm"),
  fuelLimit: z.number().int().positive().optional(),
  maxMemoryMb: z.number().int().positive().optional(),
  enabled: z.boolean().optional(),
  abis: z.array(AbiRefSchema).min(1, "At least one ABI reference is required"),
  eventHandlers: z.array(EventHandlerSchema).min(1, "At least one event handler is required"),
});

export const DataSourceSchema = z.object({
  kind: z.string().default("ethereum/contract"),
  name: z.string().regex(/^[A-Za-z][A-Za-z0-9_]*$/, "DataSource name must be alphanumeric"),
  network: z.string().min(1, "Network name is required"),
  source: z.object({
    address: z.string().regex(/^0x[0-9a-fA-F]{40}$/, "Invalid 0x hex EVM contract address"),
    abi: z.string().min(1, "ABI reference is required"),
    startBlock: z.number().int().nonnegative().default(0),
  }),
  mapping: DataSourceMappingSchema,
});

export const WebhookSchema = z.object({
  url: z.string().url("Webhook destination must be a valid URL"),
  events: z.array(z.string()).default(["reorg"]),
  maxRetries: z.number().int().nonnegative().default(3),
  secret: z.string().optional(),
});

export const LogrixConfigSchema = z.object({
  specVersion: z.string().default("0.1.0"),
  network: z.object({
    name: z.string().min(1, "Network name is required"),
    chainId: z.number().int().positive("Chain ID must be positive"),
    rpcUrl: z.string().url("Primary RPC URL must be a valid URL"),
    fallbackRpcUrls: z.array(z.string()).optional(),
  }),
  dataSources: z.array(DataSourceSchema).min(1, "At least one dataSource is required"),
  webhooks: z.array(WebhookSchema).optional(),
  indexing: z.object({
    ringBufferDepth: z.number().int().positive().default(128),
    reconcilerIntervalSecs: z.number().int().positive().default(30),
    chunkSize: z.number().int().positive().default(10),
    reconcilerChunkSize: z.number().int().positive().default(10),
    reconcilerLagThreshold: z.number().int().nonnegative().optional(),
    chainHeadPollMs: z.number().int().positive().optional(),
    maxQueueDepth: z.number().int().positive().optional(),
    rpcTimeoutSecs: z.number().int().positive().optional(),
    rpcCbThreshold: z.number().int().positive().optional(),
    rpcCooldownSecs: z.number().int().positive().optional(),
    cuBudget: z.number().int().positive().optional(),
    bulkStreamUrl: z.string().optional(),
    databaseMaxConnections: z.number().int().positive().optional(),
    databaseMinConnections: z.number().int().positive().optional(),
    rabbitmqPrefetch: z.number().int().positive().optional(),
    rabbitmqTimeoutSecs: z.number().int().positive().optional(),
    sqsWaitTimeSeconds: z.number().int().min(0).max(20).optional(),
    sqsMaxMessages: z.number().int().min(1).max(10).optional(),
    wasmFuelLimit: z.number().int().positive().optional(),
    wasmMaxMemoryMb: z.number().int().positive().optional(),
    graphqlMaxDepth: z.number().int().positive().optional(),
    graphqlMaxComplexity: z.number().int().positive().optional(),
    graphqlDefaultLimit: z.number().int().positive().optional(),
    graphqlMaxLimit: z.number().int().positive().optional(),
    graphqlQueryTimeoutSecs: z.number().int().positive().optional(),
    webhookTimeoutSecs: z.number().int().positive().optional(),
    webhookMaxRetries: z.number().int().nonnegative().optional(),
  }).passthrough().optional(),
});

export type LogrixConfig = z.infer<typeof LogrixConfigSchema>;
export type DataSource = z.infer<typeof DataSourceSchema>;
export type WebhookConfig = z.infer<typeof WebhookSchema>;
