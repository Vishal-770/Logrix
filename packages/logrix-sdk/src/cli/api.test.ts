import assert from "node:assert/strict";
import { test } from "node:test";
import {
  LogrixConfigSchema,
  convertSdkManifestToRuntime,
  resolveMappingSource,
  extractEnvVarNames,
  mapIndexingToHelmValues,
} from "./api";

test("LogrixConfigSchema strictly validates manifest types and rejects invalid chainId", () => {
  const invalidConfig = {
    specVersion: "0.8.0",
    network: {
      chainId: "not-a-number",
      rpcUrl: "https://eth.llamarpc.com",
    },
    dataSources: [],
  };

  const parsed = LogrixConfigSchema.safeParse(invalidConfig);
  assert.equal(parsed.success, false);
});

test("resolveMappingSource prefers mapping.source over mapping.file and default fallback", () => {
  const projectDir = "/tmp/mock-project";

  // Case 1: Custom mapping.source
  const configWithSource = {
    dataSources: [
      {
        mapping: {
          source: "custom/handlers/index.ts",
          file: "build/mapping.wasm",
        },
      },
    ],
  };
  assert.equal(
    resolveMappingSource(projectDir, configWithSource),
    "/tmp/mock-project/custom/handlers/index.ts"
  );

  // Case 2: Legacy mapping.file ending in .ts
  const configWithTsFile = {
    dataSources: [
      {
        mapping: {
          file: "src/indexer.ts",
        },
      },
    ],
  };
  assert.equal(
    resolveMappingSource(projectDir, configWithTsFile),
    "/tmp/mock-project/src/indexer.ts"
  );

  // Case 3: Empty / missing fallback to default src/mapping.ts
  assert.equal(
    resolveMappingSource(projectDir, {}),
    "/tmp/mock-project/src/mapping.ts"
  );
});

test("extractEnvVarNames extracts unique env variables from template strings", () => {
  const str = "https://${RPC_HOST}/v2/${RPC_KEY}?secret=${RPC_KEY}";
  const vars = extractEnvVarNames(str);
  assert.deepEqual(vars, ["RPC_HOST", "RPC_KEY"]);
});

test("mapIndexingToHelmValues maps SDK config into values-aws helm structure", () => {
  const values: any = { config: {} };
  const sdkIndexing = {
    chunkSize: 500,
    chainHeadPollMs: 250,
    maxQueueDepth: 1000,
    graphqlMaxDepth: 10,
  };

  mapIndexingToHelmValues(values, sdkIndexing);
  assert.equal(values.config.chunkSize, 500);
  assert.equal(values.config.chainHeadPollMs, 250);
  assert.equal(values.config.maxQueueDepth, 1000);
  assert.equal(values.config.graphqlMaxDepth, 10);
});

test("convertSdkManifestToRuntime preserves startBlock per contract and filters declarative webhooks", () => {
  const sdkManifest = {
    dataSources: [
      {
        name: "UniswapV3Factory",
        source: {
          address: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
          abi: "Factory",
          startBlock: 12369621,
        },
        mapping: {
          abis: [{ name: "Factory", file: "abis/factory.json" }],
          eventHandlers: [{ event: "PoolCreated", handler: "handlePoolCreated" }],
        },
      },
    ],
    webhooks: [
      {
        url: "https://my-webhook.com/endpoint",
        events: ["reorg", "block"],
        maxRetries: 5,
        secret: "my-secret-key",
      },
    ],
  };

  const converted = convertSdkManifestToRuntime(sdkManifest, 1);
  assert.equal(converted.contracts.length, 1);
  assert.equal(converted.contracts[0].address, "0x1F98431c8aD98523631AE4a59f267346ea31F984");
  assert.equal(converted.contracts[0].start_block, 12369621);
  assert.equal(converted.contracts[0].wasm_handler, "/etc/logrix/handlers/mapping.wasm");
  assert.equal(converted.webhooks.length, 1);
  assert.equal(converted.webhooks[0].max_retries, 5);
  assert.equal(converted.webhooks[0].secret, "${WEBHOOK_SECRET}");
});
