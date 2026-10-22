"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.buildCommand = buildCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const child_process_1 = require("child_process");
const picocolors_1 = __importDefault(require("picocolors"));
const yaml_1 = require("yaml");
const ui_1 = require("../ui");
const errors_1 = require("../errors");
const manifest_converter_1 = require("../manifest_converter");
function buildCommand() {
    const cmd = new commander_1.Command("build");
    cmd
        .description("Compile AssemblyScript handler to WASM using asc")
        .option("--entry <file>", "Handler entry point", "src/mapping.ts")
        .option("--out <file>", "Output WASM path", "build/mapping.wasm")
        .option("--debug", "Build without optimization (for debugging)")
        .action((opts) => {
        const projectDir = process.cwd();
        ui_1.ui.banner("Handler Compilation", "Compiling AssemblyScript mapping to WebAssembly");
        // Resolve entry and output paths
        const entryFile = path.resolve(projectDir, opts.entry);
        const outFile = path.resolve(projectDir, opts.out);
        if (!fs.existsSync(entryFile)) {
            throw new errors_1.CliError(`Entry file missing: ${opts.entry}`, `Expected handler file at ${entryFile}`, "Ensure your mapping file exists or pass --entry <path>");
        }
        // Check if generated bindings exist
        const generatedSchema = path.join(projectDir, "src", "generated", "schema.ts");
        const generatedEvents = path.join(projectDir, "src", "generated", "events.ts");
        if (!fs.existsSync(generatedSchema) || !fs.existsSync(generatedEvents)) {
            ui_1.ui.warn("Generated bindings incomplete", "Run `logrix codegen` before compiling to refresh typed event and schema classes");
        }
        // Ensure output directory exists
        fs.mkdirSync(path.dirname(outFile), { recursive: true });
        // Resolve handler source entry point
        const configPath = path.join(projectDir, "logrix.yaml");
        let resolvedEntry = entryFile;
        if (fs.existsSync(configPath)) {
            try {
                const config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
                resolvedEntry = (0, manifest_converter_1.resolveMappingSource)(projectDir, config);
            }
            catch {
                // fallback to opts.entry
            }
        }
        // Locate asc: require local node_modules compiler
        const ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
        if (!fs.existsSync(ascBin)) {
            throw new errors_1.CliError("AssemblyScript compiler (`asc`) not found", `Missing local asc binary at ${ascBin}`, "Run `npm install` in your project directory to install dependencies.");
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
        ui_1.ui.info("Target output:", picocolors_1.default.cyan(path.relative(projectDir, outFile)));
        ui_1.ui.info("Mode:", opts.debug ? picocolors_1.default.yellow("Debug (unoptimized)") : picocolors_1.default.green("Optimized (--optimize)"));
        console.log();
        const spinner = ui_1.ui.spinner("Executing AssemblyScript compiler (asc)...");
        try {
            (0, child_process_1.execSync)(ascCmd, { cwd: projectDir, stdio: "pipe" });
            const stat = fs.statSync(outFile);
            spinner.succeed(`Compiled WebAssembly binary (${(stat.size / 1024).toFixed(1)} KB)`);
            console.log();
            ui_1.ui.success("Compilation Succeeded", path.relative(projectDir, outFile));
            console.log();
        }
        catch (e) {
            spinner.fail("AssemblyScript compilation failed");
            const details = e.stderr ? e.stderr.toString() : e.message;
            throw new errors_1.CliError("Compilation Failed", details, "Check type imports in src/mapping.ts and verify generated bindings with `logrix codegen`");
        }
    });
    return cmd;
}
