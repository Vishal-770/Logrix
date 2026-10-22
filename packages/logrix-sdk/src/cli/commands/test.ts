import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";
import pc from "picocolors";
import { parse as parseYaml } from "yaml";
import { ui } from "../ui";
import { CliError } from "../errors";
import { resolveMappingSource } from "../manifest_converter";

export function testCommand(): Command {
  const cmd = new Command("test");

  cmd
    .description("Run local tests for indexer handlers, configuration, and WASM build")
    .argument("[testFile]", "Optional specific test file to run")
    .option("--watch", "Watch mode (rerun on file changes)")
    .action(async (testFile?: string) => {
      const projectDir = process.cwd();
      const configPath = path.join(projectDir, "logrix.yaml");

      ui.banner("Indexer Test Suite", "Running configuration check, build verification, and unit tests");

      if (!fs.existsSync(configPath)) {
        throw new CliError(
          "logrix.yaml not found",
          "Manifest is required to run the test suite",
          "Run `logrix test` from your indexer project root directory"
        );
      }

      let failed = false;

      // 1. Validate configuration
      const spin1 = ui.spinner("Validating configuration & schema...");
      try {
        const configContent = fs.readFileSync(configPath, "utf-8");
        const config = parseYaml(configContent);
        if (!config.dataSources || config.dataSources.length === 0) {
          throw new Error("No dataSources found in logrix.yaml");
        }
        const schemaPath = path.join(projectDir, "schema.graphql");
        if (!fs.existsSync(schemaPath)) {
          throw new Error("schema.graphql missing in project root");
        }
        spin1.succeed("Configuration check passed (logrix.yaml & schema.graphql)");
      } catch (e: any) {
        spin1.fail("Configuration check failed");
        ui.error("Configuration check failed", e.message);
        failed = true;
      }

      // 2. Compile AssemblyScript mapping
      const spin2 = ui.spinner("Compiling AssemblyScript handlers (dry-run)...");
      try {
        const ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
        const ascCmd = fs.existsSync(ascBin) ? ascBin : "npx asc";
        let mappingSrc = "src/mapping.ts";
        try {
          const config = parseYaml(fs.readFileSync(configPath, "utf-8"));
          mappingSrc = resolveMappingSource(projectDir, config);
        } catch {
          // fallback
        }
        execSync(
          `${ascCmd} ${mappingSrc} -o build/test_mapping.wasm -O3 --runtime stub --noAssert`,
          { cwd: projectDir, stdio: "pipe" }
        );
        spin2.succeed("Handler compilation passed (build/test_mapping.wasm)");
      } catch (e: any) {
        spin2.fail("Handler compilation failed");
        ui.error("Handler compilation failed", e.stderr ? e.stderr.toString() : e.message);
        failed = true;
      }

      // 3. Run user test suite if tests directory or file exists
      const testsDir = path.join(projectDir, "tests");
      const hasTestsDir = fs.existsSync(testsDir);

      if (testFile || hasTestsDir) {
        const spin3 = ui.spinner("Executing test suite runner...");
        try {
          const target = testFile
            ? path.resolve(projectDir, testFile)
            : `${testsDir}/**/*.test.{ts,js}`;
          execSync(`node --test ${target}`, {
            cwd: projectDir,
            stdio: "pipe",
          });
          spin3.succeed("All test cases passed successfully");
        } catch (e: any) {
          spin3.fail("Unit test suite reported failures");
          if (e.stdout) console.log(e.stdout.toString());
          if (e.stderr) console.error(e.stderr.toString());
          failed = true;
        }
      } else {
        ui.info("Unit tests skipped", "No tests/ directory found (create tests/*.test.ts for custom tests)");
      }

      ui.divider();
      console.log();
      if (failed) {
        throw new CliError(
          "Test Suite Failed",
          "One or more checks did not pass. Inspect the logs above.",
          "Resolve configuration or TypeScript compiler errors and run `logrix test` again."
        );
      } else {
        ui.success("All Tests Passed", "Project configuration, handlers, and unit tests verified");
        console.log();
      }
    });

  return cmd;
}
