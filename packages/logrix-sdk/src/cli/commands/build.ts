import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";
import pc from "picocolors";
import { parse as parseYaml } from "yaml";
import { ui } from "../ui";
import { CliError } from "../errors";
import { resolveMappingSource } from "../manifest_converter";

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

      ui.banner("Handler Compilation", "Compiling AssemblyScript mapping to WebAssembly");

      // Resolve entry and output paths
      const entryFile = path.resolve(projectDir, opts.entry);
      const outFile = path.resolve(projectDir, opts.out);

      if (!fs.existsSync(entryFile)) {
        throw new CliError(
          `Entry file missing: ${opts.entry}`,
          `Expected handler file at ${entryFile}`,
          "Ensure your mapping file exists or pass --entry <path>"
        );
      }

      // Check if generated bindings exist
      const generatedSchema = path.join(projectDir, "src", "generated", "schema.ts");
      const generatedEvents = path.join(projectDir, "src", "generated", "events.ts");
      if (!fs.existsSync(generatedSchema) || !fs.existsSync(generatedEvents)) {
        ui.warn(
          "Generated bindings incomplete",
          "Run `logrix codegen` before compiling to refresh typed event and schema classes"
        );
      }

      // Ensure output directory exists
      fs.mkdirSync(path.dirname(outFile), { recursive: true });

      // Resolve handler source entry point
      const configPath = path.join(projectDir, "logrix.yaml");
      let resolvedEntry = entryFile;
      if (fs.existsSync(configPath)) {
        try {
          const config = parseYaml(fs.readFileSync(configPath, "utf-8")) as any;
          resolvedEntry = resolveMappingSource(projectDir, config);
        } catch {
          // fallback to opts.entry
        }
      }

      // Locate asc: require local node_modules compiler
      const ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
      if (!fs.existsSync(ascBin)) {
        throw new CliError(
          "AssemblyScript compiler (`asc`) not found",
          `Missing local asc binary at ${ascBin}`,
          "Run `npm install` in your project directory to install dependencies."
        );
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

      ui.info("Target output:", pc.cyan(path.relative(projectDir, outFile)));
      ui.info("Mode:", opts.debug ? pc.yellow("Debug (unoptimized)") : pc.green("Optimized (--optimize)"));
      console.log();

      const spinner = ui.spinner("Executing AssemblyScript compiler (asc)...");
      try {
        execSync(ascCmd, { cwd: projectDir, stdio: "pipe" });
        const stat = fs.statSync(outFile);
        spinner.succeed(`Compiled WebAssembly binary (${(stat.size / 1024).toFixed(1)} KB)`);
        console.log();
        ui.success(
          "Compilation Succeeded",
          path.relative(projectDir, outFile)
        );
        console.log();
      } catch (e: any) {
        spinner.fail("AssemblyScript compilation failed");
        const details = e.stderr ? e.stderr.toString() : e.message;
        throw new CliError(
          "Compilation Failed",
          details,
          "Check type imports in src/mapping.ts and verify generated bindings with `logrix codegen`"
        );
      }
    });

  return cmd;
}
