import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import {
  valuesAwsTemplate,
  secretsExampleTemplate,
} from "../templates";

/**
 * logrix export-values
 *
 * Reads logrix.yaml, schema.graphql, and build/mapping.wasm then writes an
 * indexer-values.yaml that is ready to be passed to:
 *   helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
 *     -f deploy/values-local.yaml -f indexer-values.yaml
 */
export function exportValuesCommand(): Command {
  const cmd = new Command("export-values");

  cmd
    .description("Export a Helm-compatible indexer-values.yaml from this project")
    .option("--out <file>", "Output file path", "indexer-values.yaml")
    .option("--chain-id <id>", "Override chain ID (default: from logrix.yaml)")
    .option("--rpc <url>", "Override RPC URL (default: from logrix.yaml)")
    .option(
      "--deploy-profiles",
      "Regenerate deploy/ profiles (values-local.yaml, values-aws.yaml, secrets.example.yaml)"
    )
    .action((opts: { out: string; chainId?: string; rpc?: string; deployProfiles?: boolean }) => {
      const projectDir = process.cwd();
      const configPath = path.join(projectDir, "logrix.yaml");

      if (!fs.existsSync(configPath)) {
        console.error("Error: logrix.yaml not found. Run this command from your project root.");
        process.exit(1);
      }

      let config: any;
      try {
        config = parseYaml(fs.readFileSync(configPath, "utf-8"));
      } catch (e: any) {
        console.error(`Error parsing logrix.yaml: ${e.message}`);
        process.exit(1);
      }

      // Resolve network settings
      const chainId = opts.chainId
        ? parseInt(opts.chainId, 10)
        : config.network?.chainId ?? 1;
      const rpcUrl = opts.rpc ?? config.network?.rpcUrl ?? "";

      if (!rpcUrl) {
        console.error(
          "Error: RPC URL not found in logrix.yaml (network.rpcUrl) and --rpc was not passed."
        );
        process.exit(1);
      }

      // Build contracts manifest array (Rust engine format)
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

      // Build webhooks manifest array (Rust engine format)
      const webhooks: any[] = [];
      for (const wh of config.webhooks ?? []) {
        if (!wh.url) continue;
        const entry: any = {
          url: wh.url,
          events: Array.isArray(wh.events) ? wh.events : ["reorg"],
          max_retries: wh.maxRetries ?? wh.max_retries ?? 3,
        };
        if (wh.secret) {
          entry.secret = wh.secret;
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

      // Read schema.graphql
      const schemaPath = path.join(projectDir, "schema.graphql");
      if (!fs.existsSync(schemaPath)) {
        console.error("Error: schema.graphql not found. Run this command from your project root.");
        process.exit(1);
      }
      const schemaContent = fs.readFileSync(schemaPath, "utf-8");

      // Read compiled WASM (base64)
      const wasmPath = path.join(projectDir, "build", "mapping.wasm");
      if (!fs.existsSync(wasmPath)) {
        console.error(
          "Error: build/mapping.wasm not found. Run `logrix build` first."
        );
        process.exit(1);
      }
      const wasmBase64 = fs.readFileSync(wasmPath).toString("base64");

      // Resolve advanced network and engine settings
      const fallbackUrls = config.network?.fallbackRpcUrls ?? [];
      const rpcFallbackStr = Array.isArray(fallbackUrls) ? fallbackUrls.join(",") : "";
      const reconcilerIntervalSecs = config.indexing?.reconcilerIntervalSecs ?? 30;
      const ringBufferDepth = config.indexing?.ringBufferDepth ?? 128;

      // Build indexer-values.yaml
      const values: any = {
        config: {
          chainId,
          rpcUrl,
          rpcFallbackUrls: rpcFallbackStr,
          contractAddress: contracts[0]?.address ?? "",
          startBlock: String(minStartBlock),
          reconcilerIntervalSecs,
          ringBufferDepth,
          manifestContent: stringifyYaml(manifest, { indent: 2 }),
          schemaContent,
          wasmBase64,
        },
      };

      if (webhooks.length > 0) {
        values.config.webhookUrl = webhooks[0].url;
        values.webhook = {
          enabled: true,
        };
      }

      const outPath = path.resolve(projectDir, opts.out);
      fs.writeFileSync(outPath, stringifyYaml(values, { indent: 2 }));

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
          fs.writeFileSync(awsPath, valuesAwsTemplate(answers));
        }
        const secretPath = path.join(deployDir, "secrets.example.yaml");
        if (!fs.existsSync(secretPath) || opts.deployProfiles) {
          fs.writeFileSync(secretPath, secretsExampleTemplate(answers));
        }
      }

      console.log(`\nExported Helm values to: ${opts.out}`);
      console.log("\nDeploy with Helm:");
      console.log(
        `  Self-Hosted: helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f ${opts.out}`
      );
      console.log(
        `  AWS:         helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-aws.yaml -f ${opts.out} --set config.existingSecret=logrix-secrets\n`
      );
    });

  return cmd;
}
