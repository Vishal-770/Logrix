import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";
import { parse as parseYaml } from "yaml";

interface MappingSource {
  file: string;
}

interface DataSource {
  mapping: MappingSource;
}

interface LogrixConfig {
  dataSources: DataSource[];
}

export function buildCommand(): Command {
  const cmd = new Command("build");

  cmd
    .description("Compile AssemblyScript handler to WASM using asc")
    .option("--entry <file>", "Handler entry point", "src/mapping.ts")
    .option("--out <file>", "Output WASM path", "build/mapping.wasm")
    .option("--debug", "Build without optimization (for debugging)")
    .action((opts: { entry: string; out: string; debug: boolean }) => {
      const projectDir = process.cwd();

      // Resolve entry and output paths
      const entryFile = path.resolve(projectDir, opts.entry);
      const outFile = path.resolve(projectDir, opts.out);

      if (!fs.existsSync(entryFile)) {
        console.error(`Error: entry file not found: ${opts.entry}`);
        process.exit(1);
      }

      // Check if generated bindings exist
      const generatedSchema = path.join(projectDir, "src", "generated", "schema.ts");
      const generatedEvents = path.join(projectDir, "src", "generated", "events.ts");
      if (!fs.existsSync(generatedSchema) || !fs.existsSync(generatedEvents)) {
        console.warn("Notice: Generated bindings (src/generated/) not found or incomplete.");
        console.warn("Tip: Run `logrix codegen` before `logrix build` to generate typed classes.\n");
      }

      // Ensure output directory exists
      fs.mkdirSync(path.dirname(outFile), { recursive: true });

      // Prefer logrix.yaml mapping.file if present
      const configPath = path.join(projectDir, "logrix.yaml");
      let resolvedEntry = entryFile;
      if (fs.existsSync(configPath)) {
        try {
          const config = parseYaml(
            fs.readFileSync(configPath, "utf-8")
          ) as LogrixConfig;
          const mappingFile = config?.dataSources?.[0]?.mapping?.file;
          if (mappingFile) {
            const fromConfig = path.resolve(projectDir, mappingFile);
            if (fs.existsSync(fromConfig)) {
              resolvedEntry = fromConfig;
            }
          }
        } catch {
          // fallback to opts.entry
        }
      }

      // Locate asc: prefer local node_modules, fall back to global
      let ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
      if (!fs.existsSync(ascBin)) {
        ascBin = "asc"; // global
      }

      const optimizeFlag = opts.debug ? "" : "--optimize";
      const ascCmd = [
        ascBin,
        resolvedEntry,
        "--config", "tsconfig.json",
        "-o", outFile,
        optimizeFlag,
        "--exportRuntime",
      ]
        .filter(Boolean)
        .join(" ");

      console.log(`Running: ${ascCmd}\n`);

      try {
        execSync(ascCmd, { cwd: projectDir, stdio: "inherit" });
        const stat = fs.statSync(outFile);
        console.log(
          `\nBuild complete: ${path.relative(projectDir, outFile)} (${stat.size} bytes)`
        );
      } catch {
        console.error("\nBuild failed.");
        console.error("Troubleshooting tips:");
        console.error("  1. Verify AssemblyScript types in src/mapping.ts (e.g. use BigInt / Address / Bytes from @logrix/sdk)");
        console.error("  2. Run `logrix codegen` to refresh generated bindings");
        console.error("  3. Run `logrix validate` to check configuration syntax\n");
        process.exit(1);
      }
    });

  return cmd;
}
