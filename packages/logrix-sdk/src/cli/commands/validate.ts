import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { parse as parseYaml } from "yaml";

export function validateCommand(): Command {
  const cmd = new Command("validate");

  cmd
    .description("Validate project configuration, ABIs, schema, and handler files")
    .action(() => {
      const projectDir = process.cwd();
      let hasErrors = false;

      console.log("\nValidating Logrix project in:", projectDir, "\n");

      // 1. Check logrix.yaml
      const configPath = path.join(projectDir, "logrix.yaml");
      if (!fs.existsSync(configPath)) {
        console.error("  [FAIL] logrix.yaml not found.");
        process.exit(1);
      }

      let config: any;
      try {
        config = parseYaml(fs.readFileSync(configPath, "utf-8"));
        console.log("  [OK] logrix.yaml syntax valid.");
      } catch (e: any) {
        console.error(`  [FAIL] logrix.yaml syntax error: ${e.message}`);
        process.exit(1);
      }

      if (!config.dataSources || !Array.isArray(config.dataSources) || config.dataSources.length === 0) {
        console.error("  [FAIL] logrix.yaml must contain at least one dataSource in 'dataSources'.");
        hasErrors = true;
      } else {
        console.log(`  [OK] Found ${config.dataSources.length} dataSource(s).`);
      }

      // 2. Check each dataSource
      if (Array.isArray(config.dataSources)) {
        for (let i = 0; i < config.dataSources.length; i++) {
          const ds = config.dataSources[i];
          const dsName = ds.name || `dataSource[${i}]`;

          if (!ds.name) {
            console.error(`  [FAIL] ${dsName}: missing 'name' attribute.`);
            hasErrors = true;
          }

          const addr = ds.source?.address;
          if (!addr || !/^0x[0-9a-fA-F]{40}$/.test(addr)) {
            console.error(`  [FAIL] ${dsName}: source.address '${addr}' is not a valid 40-character 0x hex address.`);
            hasErrors = true;
          } else {
            console.log(`  [OK] ${dsName}: contract address valid (${addr}).`);
          }

          // Check ABIs
          const abis = ds.mapping?.abis || [];
          if (!Array.isArray(abis) || abis.length === 0) {
            console.error(`  [FAIL] ${dsName}: mapping.abis must list at least one ABI file.`);
            hasErrors = true;
          } else {
            for (const abiRef of abis) {
              const abiFile = path.resolve(projectDir, abiRef.file);
              if (!fs.existsSync(abiFile)) {
                console.error(`  [FAIL] ${dsName}: ABI file not found at ${abiRef.file}`);
                hasErrors = true;
              } else {
                try {
                  const parsed = JSON.parse(fs.readFileSync(abiFile, "utf-8"));
                  if (!Array.isArray(parsed)) {
                    console.error(`  [FAIL] ${dsName}: ABI file ${abiRef.file} is not a JSON array.`);
                    hasErrors = true;
                  } else {
                    const eventCount = parsed.filter((item: any) => item.type === "event").length;
                    console.log(`  [OK] ${dsName}: ABI ${abiRef.name} loaded with ${eventCount} event definition(s).`);
                  }
                } catch (e: any) {
                  console.error(`  [FAIL] ${dsName}: ABI file ${abiRef.file} is invalid JSON: ${e.message}`);
                  hasErrors = true;
                }
              }
            }
          }

          // Check handler mapping file
          const mappingFile = path.resolve(projectDir, ds.mapping?.file || "src/mapping.ts");
          const mappingSrc = path.resolve(projectDir, "src/mapping.ts");
          if (!fs.existsSync(mappingSrc)) {
            console.error(`  [FAIL] ${dsName}: handler source file not found at src/mapping.ts`);
            hasErrors = true;
          } else {
            console.log(`  [OK] ${dsName}: handler entry point exists at src/mapping.ts`);
          }
        }
      }

      // 3. Check schema.graphql
      const schemaPath = path.join(projectDir, "schema.graphql");
      if (!fs.existsSync(schemaPath)) {
        console.error("  [FAIL] schema.graphql not found.");
        hasErrors = true;
      } else {
        const schemaContent = fs.readFileSync(schemaPath, "utf-8");
        const entityCount = (schemaContent.match(/type\s+\w+\s+@entity/g) || []).length;
        if (entityCount === 0) {
          console.error("  [FAIL] schema.graphql has no @entity types declared.");
          hasErrors = true;
        } else {
          console.log(`  [OK] schema.graphql valid with ${entityCount} @entity definition(s).`);
        }
      }

      console.log("");
      if (hasErrors) {
        console.error("Validation failed. Please address the errors above before building.\n");
        process.exit(1);
      } else {
        console.log("Validation passed successfully! All configuration files, ABIs, and schemas are valid.\n");
      }
    });

  return cmd;
}
