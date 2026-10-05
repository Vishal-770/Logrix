import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";
import { parse as parseYaml } from "yaml";

export function testCommand(): Command {
  const cmd = new Command("test");

  cmd
    .description("Run local tests for indexer handlers, configuration, and WASM build")
    .argument("[testFile]", "Optional specific test file to run")
    .option("--watch", "Watch mode (rerun on file changes)")
    .action(async (testFile?: string, opts?: { watch?: boolean }) => {
      const projectDir = process.cwd();
      const configPath = path.join(projectDir, "logrix.yaml");

      console.log("\nLogrix Test Runner");
      console.log("------------------------------------------");

      if (!fs.existsSync(configPath)) {
        console.error("Error: logrix.yaml not found. Run `logrix test` from your project root.");
        process.exit(1);
      }

      let failed = false;

      // 1. Validate configuration
      process.stdout.write("  [1/3] Validating configuration & schema... ");
      try {
        const configContent = fs.readFileSync(configPath, "utf-8");
        const config = parseYaml(configContent);
        if (!config.dataSources || config.dataSources.length === 0) {
          throw new Error("No dataSources found in logrix.yaml");
        }
        const schemaPath = path.join(projectDir, "schema.graphql");
        if (!fs.existsSync(schemaPath)) {
          throw new Error("schema.graphql missing");
        }
        console.log("PASSED");
      } catch (e: any) {
        console.log("FAILED");
        console.error(`        ${e.message}`);
        failed = true;
      }

      // 2. Compile AssemblyScript mapping
      process.stdout.write("  [2/3] Compiling AssemblyScript handlers... ");
      try {
        const ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
        const ascCmd = fs.existsSync(ascBin) ? ascBin : "npx asc";
        execSync(
          `${ascCmd} src/mapping.ts -o build/test_mapping.wasm -O3 --runtime stub --noAssert`,
          { cwd: projectDir, stdio: "pipe" }
        );
        console.log("PASSED");
      } catch (e: any) {
        console.log("FAILED");
        console.error(`        ${e.stderr ? e.stderr.toString() : e.message}`);
        failed = true;
      }

      // 3. Run user test suite if tests directory or file exists
      const testsDir = path.join(projectDir, "tests");
      const hasTestsDir = fs.existsSync(testsDir);

      if (testFile || hasTestsDir) {
        process.stdout.write("  [3/3] Running handler unit tests... ");
        try {
          const target = testFile
            ? path.resolve(projectDir, testFile)
            : `${testsDir}/**/*.test.{ts,js}`;
          execSync(`node --test ${target}`, {
            cwd: projectDir,
            stdio: "inherit",
          });
          console.log("PASSED");
        } catch {
          console.log("FAILED");
          failed = true;
        }
      } else {
        console.log("  [3/3] Handler unit tests... SKIPPED (no tests/ directory found)");
        console.log("        Create tests in tests/*.test.ts to add custom unit tests.");
      }

      console.log("------------------------------------------");
      if (failed) {
        console.error("Result: TESTS FAILED\n");
        process.exit(1);
      } else {
        console.log("Result: ALL CHECKS & TESTS PASSED\n");
      }
    });

  return cmd;
}
