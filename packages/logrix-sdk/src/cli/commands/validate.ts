import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import pc from "picocolors";
import { parse as parseYaml } from "yaml";
import { ui } from "../ui";
import { LogrixConfigSchema } from "../schema";
import { CliError } from "../errors";

export function validateCommand(): Command {
  const cmd = new Command("validate");

  cmd
    .description("Validate project configuration, ABIs, schema, and handler files")
    .action(() => {
      const projectDir = process.cwd();
      let totalChecks = 0;
      let passedChecks = 0;
      let hasErrors = false;

      ui.banner("Project Validation", "Verifying manifest, ABIs, schema, and handlers");

      // 1. Check logrix.yaml
      ui.section("Configuration Manifest");
      const configPath = path.join(projectDir, "logrix.yaml");
      totalChecks++;
      if (!fs.existsSync(configPath)) {
        throw new CliError(
          "Manifest missing: logrix.yaml not found",
          "Project manifest is missing in current working directory",
          "Run `logrix init` to scaffold a new project"
        );
      } else {
        passedChecks++;
      }

      let config: any;
      totalChecks++;
      try {
        config = parseYaml(fs.readFileSync(configPath, "utf-8"));
        ui.success("Manifest syntax", "logrix.yaml is valid YAML");
        passedChecks++;
      } catch (e: any) {
        throw new CliError("Manifest syntax error", e.message);
      }

      totalChecks++;
      const zodResult = LogrixConfigSchema.safeParse(config);
      if (!zodResult.success) {
        const issues = zodResult.error.issues
          .map((i) => `${i.path.join(".")}: ${i.message}`)
          .join(", ");
        ui.error("Zod Schema Validation Failed", issues);
        hasErrors = true;
      } else {
        ui.success("Schema definition valid", "Manifest matches typed Logrix specification");
        passedChecks++;
      }

      totalChecks++;
      if (!config.dataSources || !Array.isArray(config.dataSources) || config.dataSources.length === 0) {
        ui.error("Missing data sources", "logrix.yaml must declare at least one dataSource");
        hasErrors = true;
      } else {
        ui.success("Data sources configured", `${config.dataSources.length} contract source(s) declared`);
        passedChecks++;
      }

      // 2. Check each dataSource
      if (Array.isArray(config.dataSources)) {
        ui.section("Contract Sources & ABIs");
        for (let i = 0; i < config.dataSources.length; i++) {
          const ds = config.dataSources[i];
          const dsName = ds.name || `dataSource[${i}]`;

          totalChecks++;
          if (!ds.name) {
            ui.error(`Source [${i}] unnamed`, "Each dataSource must specify a name");
            hasErrors = true;
          } else {
            passedChecks++;
          }

          totalChecks++;
          const addr = ds.source?.address;
          if (!addr || !/^0x[0-9a-fA-F]{40}$/.test(addr)) {
            ui.error(`${dsName}: Invalid contract address`, `'${addr}' is not a valid 40-character 0x hex address`);
            hasErrors = true;
          } else {
            ui.success(`${dsName}: Contract address`, addr);
            passedChecks++;
          }

          // Check ABIs
          const abis = ds.mapping?.abis || [];
          totalChecks++;
          if (!Array.isArray(abis) || abis.length === 0) {
            ui.error(`${dsName}: Missing ABI reference`, "mapping.abis must specify at least one ABI JSON file");
            hasErrors = true;
          } else {
            for (const abiRef of abis) {
              const abiFile = path.resolve(projectDir, abiRef.file);
              totalChecks++;
              if (!fs.existsSync(abiFile)) {
                ui.error(`${dsName}: ABI file missing`, `File not found at ${abiRef.file}`);
                hasErrors = true;
              } else {
                try {
                  const parsed = JSON.parse(fs.readFileSync(abiFile, "utf-8"));
                  if (!Array.isArray(parsed)) {
                    ui.error(`${dsName}: Invalid ABI format`, `${abiRef.file} is not a valid JSON array`);
                    hasErrors = true;
                  } else {
                    const eventCount = parsed.filter((item: any) => item.type === "event").length;
                    ui.success(`${dsName}: ABI ${abiRef.name}`, `${eventCount} event definition(s) loaded`);
                    passedChecks++;
                  }
                } catch (e: any) {
                  ui.error(`${dsName}: ABI parsing failed`, `${abiRef.file}: ${e.message}`);
                  hasErrors = true;
                }
              }
            }
          }

          // Check handler mapping file
          totalChecks++;
          const mappingSrc = path.resolve(projectDir, "src/mapping.ts");
          if (!fs.existsSync(mappingSrc)) {
            ui.error(`${dsName}: Handler missing`, "Entry point not found at src/mapping.ts");
            hasErrors = true;
          } else {
            ui.success(`${dsName}: Handler entry`, "src/mapping.ts exists");
            passedChecks++;
          }

          // Check resource limits if specified
          if (ds.mapping?.fuelLimit !== undefined) {
            totalChecks++;
            const fuel = ds.mapping.fuelLimit;
            if (typeof fuel !== "number" || fuel < 1000) {
              ui.error(`${dsName}: Invalid fuel limit`, `mapping.fuelLimit must be a number >= 1000 (got ${fuel})`);
              hasErrors = true;
            } else {
              ui.success(`${dsName}: Fuel limit`, `${fuel.toLocaleString()} ticks`);
              passedChecks++;
            }
          }

          if (ds.mapping?.maxMemoryMb !== undefined) {
            totalChecks++;
            const mem = ds.mapping.maxMemoryMb;
            if (typeof mem !== "number" || mem < 1 || mem > 512) {
              ui.error(`${dsName}: Invalid memory ceiling`, `mapping.maxMemoryMb must be between 1 and 512 MB (got ${mem})`);
              hasErrors = true;
            } else {
              ui.success(`${dsName}: Memory ceiling`, `${mem} MB`);
              passedChecks++;
            }
          }
        }
      }

      // 3. Check network fallback RPC URLs
      if (config.network?.fallbackRpcUrls) {
        ui.section("Network & Failover");
        totalChecks++;
        if (!Array.isArray(config.network.fallbackRpcUrls)) {
          ui.error("Invalid fallbackRpcUrls", "Must be a list of URL strings");
          hasErrors = true;
        } else {
          for (const url of config.network.fallbackRpcUrls) {
            totalChecks++;
            if (!/^https?:\/\/.+/.test(url)) {
              ui.error("Invalid fallback URL", `'${url}' must start with http:// or https://`);
              hasErrors = true;
            } else {
              passedChecks++;
            }
          }
          ui.success("Fallback RPC endpoints", `${config.network.fallbackRpcUrls.length} URL(s) configured`);
          passedChecks++;
        }
      }

      // 4. Check webhooks
      if (config.webhooks) {
        ui.section("Declarative Webhooks");
        totalChecks++;
        if (!Array.isArray(config.webhooks)) {
          ui.error("Invalid webhooks format", "Must be an array of webhook objects");
          hasErrors = true;
        } else {
          for (let i = 0; i < config.webhooks.length; i++) {
            const wh = config.webhooks[i];
            totalChecks++;
            if (!wh.url || !/^https?:\/\/.+/.test(wh.url)) {
              ui.error(`Webhook [${i}] invalid URL`, "Must start with http:// or https://");
              hasErrors = true;
            } else {
              passedChecks++;
            }
          }
          ui.success("Webhooks verified", `${config.webhooks.length} subscriber endpoint(s) declared`);
          passedChecks++;
        }
      }

      // 5. Check schema.graphql
      ui.section("GraphQL Schema");
      const schemaPath = path.join(projectDir, "schema.graphql");
      totalChecks++;
      if (!fs.existsSync(schemaPath)) {
        ui.error("Schema file missing", "schema.graphql not found in project root");
        hasErrors = true;
      } else {
        const schemaContent = fs.readFileSync(schemaPath, "utf-8");
        const entityCount = (schemaContent.match(/type\s+\w+\s+@entity/g) || []).length;
        if (entityCount === 0) {
          ui.error("No entities defined", "schema.graphql must declare at least one type with @entity");
          hasErrors = true;
        } else {
          ui.success("Schema valid", `${entityCount} @entity definition(s) declared`);
          passedChecks++;
        }
      }

      ui.divider();
      console.log();
      if (hasErrors) {
        ui.error(
          "Validation Failed",
          `Passed ${passedChecks} of ${totalChecks} checks. Address the errors above before compiling.`
        );
        console.log();
        process.exit(1);
      } else {
        ui.success(
          "Validation Passed",
          `All ${passedChecks} checks completed successfully. Ready for compilation.`
        );
        console.log();
      }
    });

  return cmd;
}
