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
const ui_1 = require("../ui");
const errors_1 = require("../errors");
function testCommand() {
    const cmd = new commander_1.Command("test");
    cmd
        .description("Run local tests for indexer handlers, configuration, and WASM build")
        .argument("[testFile]", "Optional specific test file to run")
        .option("--watch", "Watch mode (rerun on file changes)")
        .action(async (testFile) => {
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");
        ui_1.ui.banner("Indexer Test Suite", "Running configuration check, build verification, and unit tests");
        if (!fs.existsSync(configPath)) {
            throw new errors_1.CliError("logrix.yaml not found", "Manifest is required to run the test suite", "Run `logrix test` from your indexer project root directory");
        }
        let failed = false;
        // 1. Validate configuration
        const spin1 = ui_1.ui.spinner("Validating configuration & schema...");
        try {
            const configContent = fs.readFileSync(configPath, "utf-8");
            const config = (0, yaml_1.parse)(configContent);
            if (!config.dataSources || config.dataSources.length === 0) {
                throw new Error("No dataSources found in logrix.yaml");
            }
            const schemaPath = path.join(projectDir, "schema.graphql");
            if (!fs.existsSync(schemaPath)) {
                throw new Error("schema.graphql missing in project root");
            }
            spin1.succeed("Configuration check passed (logrix.yaml & schema.graphql)");
        }
        catch (e) {
            spin1.fail("Configuration check failed");
            ui_1.ui.error("Configuration check failed", e.message);
            failed = true;
        }
        // 2. Compile AssemblyScript mapping
        const spin2 = ui_1.ui.spinner("Compiling AssemblyScript handlers (dry-run)...");
        try {
            const ascBin = path.join(projectDir, "node_modules", ".bin", "asc");
            const ascCmd = fs.existsSync(ascBin) ? ascBin : "npx asc";
            (0, child_process_1.execSync)(`${ascCmd} src/mapping.ts -o build/test_mapping.wasm -O3 --runtime stub --noAssert`, { cwd: projectDir, stdio: "pipe" });
            spin2.succeed("Handler compilation passed (build/test_mapping.wasm)");
        }
        catch (e) {
            spin2.fail("Handler compilation failed");
            ui_1.ui.error("Handler compilation failed", e.stderr ? e.stderr.toString() : e.message);
            failed = true;
        }
        // 3. Run user test suite if tests directory or file exists
        const testsDir = path.join(projectDir, "tests");
        const hasTestsDir = fs.existsSync(testsDir);
        if (testFile || hasTestsDir) {
            const spin3 = ui_1.ui.spinner("Executing test suite runner...");
            try {
                const target = testFile
                    ? path.resolve(projectDir, testFile)
                    : `${testsDir}/**/*.test.{ts,js}`;
                (0, child_process_1.execSync)(`node --test ${target}`, {
                    cwd: projectDir,
                    stdio: "pipe",
                });
                spin3.succeed("All test cases passed successfully");
            }
            catch (e) {
                spin3.fail("Unit test suite reported failures");
                if (e.stdout)
                    console.log(e.stdout.toString());
                if (e.stderr)
                    console.error(e.stderr.toString());
                failed = true;
            }
        }
        else {
            ui_1.ui.info("Unit tests skipped", "No tests/ directory found (create tests/*.test.ts for custom tests)");
        }
        ui_1.ui.divider();
        console.log();
        if (failed) {
            throw new errors_1.CliError("Test Suite Failed", "One or more checks did not pass. Inspect the logs above.", "Resolve configuration or TypeScript compiler errors and run `logrix test` again.");
        }
        else {
            ui_1.ui.success("All Tests Passed", "Project configuration, handlers, and unit tests verified");
            console.log();
        }
    });
    return cmd;
}
