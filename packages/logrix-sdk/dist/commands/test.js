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
exports.testCommand = testCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const child_process_1 = require("child_process");
const yaml_1 = require("yaml");
function testCommand() {
    const cmd = new commander_1.Command("test");
    cmd
        .description("Run local tests for indexer handlers, configuration, and WASM build")
        .argument("[testFile]", "Optional specific test file to run")
        .option("--watch", "Watch mode (rerun on file changes)")
        .action(async (testFile, opts) => {
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
            const config = (0, yaml_1.parse)(configContent);
            if (!config.dataSources || config.dataSources.length === 0) {
                throw new Error("No dataSources found in logrix.yaml");
            }
            const schemaPath = path.join(projectDir, "schema.graphql");
            if (!fs.existsSync(schemaPath)) {
                throw new Error("schema.graphql missing");
            }
            console.log("PASSED");
        }
        catch (e) {
            console.log("FAILED");
            console.error(`        ${e.message}`);
            failed = true;
        }
        // 2. Compile AssemblyScript mapping
        process.stdout.write("  [2/3] Compiling AssemblyScript handlers... ");
        try {
            const ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
            const ascCmd = fs.existsSync(ascBin) ? ascBin : "npx asc";
            (0, child_process_1.execSync)(`${ascCmd} src/mapping.ts -o build/test_mapping.wasm -O3 --runtime stub --noAssert`, { cwd: projectDir, stdio: "pipe" });
            console.log("PASSED");
        }
        catch (e) {
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
                (0, child_process_1.execSync)(`node --test ${target}`, {
                    cwd: projectDir,
                    stdio: "inherit",
                });
                console.log("PASSED");
            }
            catch {
                console.log("FAILED");
                failed = true;
            }
        }
        else {
            console.log("  [3/3] Handler unit tests... SKIPPED (no tests/ directory found)");
            console.log("        Create tests in tests/*.test.ts to add custom unit tests.");
        }
        console.log("------------------------------------------");
        if (failed) {
            console.error("Result: TESTS FAILED\n");
            process.exit(1);
        }
        else {
            console.log("Result: ALL CHECKS & TESTS PASSED\n");
        }
    });
    return cmd;
}
