const ENV_REFERENCE_PATTERN = /\$\{([A-Za-z_][A-Za-z0-9_]*)\}/g;
const PURE_ENV_REFERENCE_PATTERN = /^\$\{[A-Za-z_][A-Za-z0-9_]*\}$/;

/**
 * Keep environment references symbolic while generating deployment artifacts.
 * The runtime pod resolves them after loading its environment.
 */
export function preserveEnvReference(val: string): string {
  return val;
}

/** Return environment variable names referenced by strings in a value, in source order. */
export function extractEnvVarNames(value: unknown): string[] {
  const names = new Set<string>();
  const collect = (candidate: unknown): void => {
    if (typeof candidate === "string") {
      for (const match of candidate.matchAll(ENV_REFERENCE_PATTERN)) {
        names.add(match[1]);
      }
      return;
    }
    if (Array.isArray(candidate)) {
      for (const item of candidate) collect(item);
      return;
    }
    if (candidate && typeof candidate === "object") {
      for (const item of Object.values(candidate)) collect(item);
    }
  };

  collect(value);
  return [...names];
}

function webhookSecretReference(secret: string, webhookIndex: number): {
  value: string;
  envVars: string[];
} {
  if (PURE_ENV_REFERENCE_PATTERN.test(secret)) {
    return {
      value: preserveEnvReference(secret),
      envVars: extractEnvVarNames(secret),
    };
  }

  const envVar = webhookIndex === 0 ? "WEBHOOK_SECRET" : `WEBHOOK_SECRET_${webhookIndex + 1}`;
  return {
    value: `\${${envVar}}`,
    envVars: [envVar],
  };
}

export interface ConvertedRuntimeManifest {
  contracts: any[];
  webhooks: any[];
  minStartBlock: number;
  manifest: any;
  requiredEnvVars: string[];
}

/**
 * Converts high-level SDK project manifest to concrete Rust runtime engine format.
 */
export function convertSdkManifestToRuntime(
  config: any,
  chainId: number
): ConvertedRuntimeManifest {
  const contracts: any[] = [];
  let minStartBlock = Number.MAX_SAFE_INTEGER;

  for (const ds of config.dataSources ?? []) {
    const events = (ds.mapping?.eventHandlers ?? []).map((h: any) => h.event);
    const startBlock = ds.source?.startBlock ?? 0;
    if (startBlock < minStartBlock) {
      minStartBlock = startBlock;
    }

    const contractEntry: any = {
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

  const webhooks: any[] = [];
  const requiredEnvVars = new Set<string>();
  for (const [webhookIndex, wh] of (config.webhooks ?? []).entries()) {
    if (!wh.url) continue;
    const entry: any = {
      url: preserveEnvReference(wh.url),
      events: Array.isArray(wh.events) ? wh.events : ["reorg"],
      max_retries: wh.maxRetries ?? wh.max_retries ?? 3,
    };

    for (const envVar of extractEnvVarNames(entry.url)) {
      requiredEnvVars.add(envVar);
    }

    if (wh.secret) {
      const secret = webhookSecretReference(wh.secret, webhookIndex);
      entry.secret = secret.value;
      for (const envVar of secret.envVars) {
        requiredEnvVars.add(envVar);
      }
    }
    webhooks.push(entry);
  }

  const manifest: any = {
    schema_version: "1",
    chain_id: chainId,
    contracts,
  };
  if (webhooks.length > 0) {
    manifest.webhooks = webhooks;
  }

  return {
    contracts,
    webhooks,
    minStartBlock,
    manifest,
    requiredEnvVars: [...requiredEnvVars],
  };
}

/**
 * Maps optional SDK indexing settings to Helm deployment values.
 */
export function mapIndexingToHelmValues(values: any, idx: any = {}): void {
  const mapping: Array<[string, string]> = [
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
