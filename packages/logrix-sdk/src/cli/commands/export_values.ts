import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";

/**
 * logrix export-values
 *
 * Reads logrix.yaml, schema.graphql, and build/mapping.wasm then writes an
 * indexer-values.yaml that is ready to be passed to:
 *   helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
 *     -f cluster-infra.yaml -f indexer-values.yaml
 */
export function exportValuesCommand(): Command {
  const cmd = new Command("export-values");

  cmd
    .description("Export a Helm-compatible indexer-values.yaml from this project")
    .option("--out <file>", "Output file path", "indexer-values.yaml")
    .option("--chain-id <id>", "Override chain ID (default: from logrix.yaml)")
    .option("--rpc <url>", "Override RPC URL (default: from logrix.yaml)")
    .action((opts: { out: string; chainId?: string; rpc?: string }) => {
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
      for (const ds of config.dataSources ?? []) {
        const events = (ds.mapping?.eventHandlers ?? []).map((h: any) => h.event);
        contracts.push({
          name: ds.name,
          address: ds.source?.address ?? "",
          events,
          wasm_handler: "./mapping.wasm",
        });
      }

      const manifest = {
        schema_version: "1",
        chain_id: chainId,
        contracts,
      };

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

      // Build indexer-values.yaml
      const values = {
        config: {
          chainId,
          rpcUrl,
          manifestContent: stringifyYaml(manifest, { indent: 2 }),
          schemaContent,
          wasmBase64,
        },
      };

      const outPath = path.resolve(projectDir, opts.out);
      fs.writeFileSync(outPath, stringifyYaml(values, { indent: 2 }));

      console.log(`\nExported Helm values to: ${opts.out}`);
      console.log("\nDeploy with:");
      console.log(
        `  helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f cluster-infra.yaml -f ${opts.out}\n`
      );
    });

  return cmd;
}
