import assert from "node:assert/strict";
import { test } from "node:test";
import {
  convertSdkManifestToRuntime,
  preserveEnvReference,
} from "./manifest_converter";
import { secretsExampleTemplate } from "./templates";

test("preserveEnvReference does not read process.env", () => {
  const variableName = "LOGRIX_SDK_TEST_SECRET";
  const originalValue = process.env[variableName];
  process.env[variableName] = "DO_NOT_SERIALIZE";

  try {
    assert.equal(
      preserveEnvReference(`https://rpc.example/${"${"}${variableName}}`),
      `https://rpc.example/${"${"}${variableName}}`
    );
  } finally {
    if (originalValue === undefined) {
      delete process.env[variableName];
    } else {
      process.env[variableName] = originalValue;
    }
  }
});

test("direct webhook secrets become references without entering the runtime manifest", () => {
  const directSecret = "DO_NOT_SERIALIZE_WEBHOOK_SECRET";
  const converted = convertSdkManifestToRuntime(
    {
      dataSources: [],
      webhooks: [
        {
          url: "https://hooks.example/events/${WEBHOOK_HOST}",
          secret: directSecret,
        },
      ],
    },
    1
  );

  assert.equal(converted.webhooks[0].secret, "${WEBHOOK_SECRET}");
  assert.deepEqual(converted.requiredEnvVars, ["WEBHOOK_HOST", "WEBHOOK_SECRET"]);
  assert.equal(JSON.stringify(converted).includes(directSecret), false);
});

test("secret examples contain placeholders instead of source values", () => {
  const sourceRpcUrl = "https://rpc.example/${ALCHEMY_API_KEY}";
  const sourceFallbackUrl = "https://fallback.example/${FALLBACK_API_KEY}";
  const output = secretsExampleTemplate({
    projectName: "example",
    networkName: "mainnet",
    chainId: 1,
    rpcUrl: sourceRpcUrl,
    fallbackRpcUrls: [sourceFallbackUrl],
    contractName: "Contract",
    contractAddress: "0x0000000000000000000000000000000000000000",
    startBlock: 0,
    secretEnvVars: ["ALCHEMY_API_KEY", "FALLBACK_API_KEY", "WEBHOOK_SECRET"],
  });

  assert.equal(output.includes(sourceRpcUrl), false);
  assert.equal(output.includes(sourceFallbackUrl), false);
  assert.equal(output.includes("ALCHEMY_API_KEY: CHANGE_ME"), true);
  assert.equal(output.includes("FALLBACK_API_KEY: CHANGE_ME"), true);
  assert.equal(output.includes("WEBHOOK_SECRET: CHANGE_ME"), true);
  assert.equal(output.includes("RPC_URL: CHANGE_ME"), true);
});
