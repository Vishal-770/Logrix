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
Object.defineProperty(exports, "__esModule", { value: true });
exports.buildCommand = buildCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const child_process_1 = require("child_process");
const yaml_1 = require("yaml");
function buildCommand() {
    const cmd = new commander_1.Command("build");
    cmd
        .description("Compile AssemblyScript handler to WASM using asc")
        .option("--entry <file>", "Handler entry point", "src/mapping.ts")
        .option("--out <file>", "Output WASM path", "build/mapping.wasm")
        .option("--debug", "Build without optimization (for debugging)")
        .action((opts) => {
        const projectDir = process.cwd();
        // Resolve entry and output paths
        const entryFile = path.resolve(projectDir, opts.entry);
        const outFile = path.resolve(projectDir, opts.out);
        if (!fs.existsSync(entryFile)) {
            console.error(`Error: entry file not found: ${opts.entry}`);
            process.exit(1);
        }
        // Ensure output directory exists
        fs.mkdirSync(path.dirname(outFile), { recursive: true });
        // Prefer logrix.yaml mapping.file if present
        const configPath = path.join(projectDir, "logrix.yaml");
        let resolvedEntry = entryFile;
        if (fs.existsSync(configPath)) {
            try {
                const config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
                const mappingFile = config?.dataSources?.[0]?.mapping?.file;
                if (mappingFile) {
                    const fromConfig = path.resolve(projectDir, mappingFile);
                    if (fs.existsSync(fromConfig)) {
                        resolvedEntry = fromConfig;
                    }
                }
            }
            catch {
                // fallback to opts.entry
            }
        }
        // Locate asc: prefer local node_modules, fall back to global
        let ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
        if (!fs.existsSync(ascBin)) {
            ascBin = "asc"; // global
        }
        const optimizeFlag = opts.debug ? "" : "--optimize";
        const cmd = [
            ascBin,
            resolvedEntry,
            "--config", "tsconfig.json",
            "-o", outFile,
            optimizeFlag,
            "--exportRuntime",
        ]
            .filter(Boolean)
            .join(" ");
        console.log(`Running: ${cmd}\n`);
        try {
            (0, child_process_1.execSync)(cmd, { cwd: projectDir, stdio: "inherit" });
            const stat = fs.statSync(outFile);
            console.log(`\nBuild complete: ${path.relative(projectDir, outFile)} (${stat.size} bytes)`);
        }
        catch {
            console.error("\nBuild failed.");
            process.exit(1);
        }
    });
    return cmd;
}
