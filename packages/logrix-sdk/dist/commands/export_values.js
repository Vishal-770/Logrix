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
exports.exportValuesCommand = exportValuesCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const yaml_1 = require("yaml");
const templates_1 = require("../templates");
/**
 * logrix export-values
 *
 * Reads logrix.yaml, schema.graphql, and build/mapping.wasm then writes an
 * indexer-values.yaml that is ready to be passed to:
 *   helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
 *     -f deploy/values-local.yaml -f indexer-values.yaml
 */
function exportValuesCommand() {
    const cmd = new commander_1.Command("export-values");
    cmd
        .description("Export a Helm-compatible indexer-values.yaml from this project")
        .option("--out <file>", "Output file path", "indexer-values.yaml")
        .option("--chain-id <id>", "Override chain ID (default: from logrix.yaml)")
        .option("--rpc <url>", "Override RPC URL (default: from logrix.yaml)")
        .option("--deploy-profiles", "Regenerate deploy/ profiles (values-local.yaml, values-aws.yaml, secrets.example.yaml)")
        .action((opts) => {
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");
        if (!fs.existsSync(configPath)) {
            console.error("Error: logrix.yaml not found. Run this command from your project root.");
            process.exit(1);
        }
        let config;
        try {
            config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
        }
        catch (e) {
            console.error(`Error parsing logrix.yaml: ${e.message}`);
            process.exit(1);
        }
        // Resolve network settings
        const chainId = opts.chainId
            ? parseInt(opts.chainId, 10)
            : config.network?.chainId ?? 1;
        const rpcUrl = opts.rpc ?? config.network?.rpcUrl ?? "";
        if (!rpcUrl) {
            console.error("Error: RPC URL not found in logrix.yaml (network.rpcUrl) and --rpc was not passed.");
            process.exit(1);
        }
        // Build contracts manifest array (Rust engine format)
        const contracts = [];
        for (const ds of config.dataSources ?? []) {
            const events = (ds.mapping?.eventHandlers ?? []).map((h) => h.event);
            contracts.push({
                name: ds.name,
                address: ds.source?.address ?? "",
                events,
                wasm_handler: "./mapping.wasm",
            });
        }
        const manifest = {
            schema_version: "1",
            chain_id: chainId,
            contracts,
        };
        // Read schema.graphql
        const schemaPath = path.join(projectDir, "schema.graphql");
        if (!fs.existsSync(schemaPath)) {
            console.error("Error: schema.graphql not found. Run this command from your project root.");
            process.exit(1);
        }
        const schemaContent = fs.readFileSync(schemaPath, "utf-8");
        // Read compiled WASM (base64)
        const wasmPath = path.join(projectDir, "build", "mapping.wasm");
        if (!fs.existsSync(wasmPath)) {
            console.error("Error: build/mapping.wasm not found. Run `logrix build` first.");
            process.exit(1);
        }
        const wasmBase64 = fs.readFileSync(wasmPath).toString("base64");
        // Build indexer-values.yaml
        const values = {
            config: {
                chainId,
                rpcUrl,
                manifestContent: (0, yaml_1.stringify)(manifest, { indent: 2 }),
                schemaContent,
                wasmBase64,
            },
        };
        const outPath = path.resolve(projectDir, opts.out);
        fs.writeFileSync(outPath, (0, yaml_1.stringify)(values, { indent: 2 }));
        // Ensure deployment profile directory exists
        const deployDir = path.join(projectDir, "deploy");
        if (!fs.existsSync(deployDir) || opts.deployProfiles) {
            fs.mkdirSync(deployDir, { recursive: true });
            const answers = {
                projectName: path.basename(projectDir),
                networkName: config.network?.name ?? "network",
                chainId,
                rpcUrl,
                contractName: contracts[0]?.name ?? "Contract",
                contractAddress: contracts[0]?.address ?? "",
                startBlock: 0,
            };
            const localPath = path.join(deployDir, "values-local.yaml");
            if (!fs.existsSync(localPath) || opts.deployProfiles) {
                fs.writeFileSync(localPath, (0, templates_1.valuesLocalTemplate)(answers));
            }
            const awsPath = path.join(deployDir, "values-aws.yaml");
            if (!fs.existsSync(awsPath) || opts.deployProfiles) {
                fs.writeFileSync(awsPath, (0, templates_1.valuesAwsTemplate)(answers));
            }
            const secretPath = path.join(deployDir, "secrets.example.yaml");
            if (!fs.existsSync(secretPath) || opts.deployProfiles) {
                fs.writeFileSync(secretPath, (0, templates_1.secretsExampleTemplate)(answers));
            }
        }
        console.log(`\nExported Helm values to: ${opts.out}`);
        console.log("\nDeploy with Helm:");
        console.log(`  Local: helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-local.yaml -f ${opts.out}`);
        console.log(`  AWS:   helm install logrix oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-aws.yaml -f ${opts.out}\n`);
    });
    return cmd;
}
