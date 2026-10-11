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
exports.validateCommand = validateCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const yaml_1 = require("yaml");
function validateCommand() {
    const cmd = new commander_1.Command("validate");
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
        let config;
        try {
            config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
            console.log("  [OK] logrix.yaml syntax valid.");
        }
        catch (e) {
            console.error(`  [FAIL] logrix.yaml syntax error: ${e.message}`);
            process.exit(1);
        }
        if (!config.dataSources || !Array.isArray(config.dataSources) || config.dataSources.length === 0) {
            console.error("  [FAIL] logrix.yaml must contain at least one dataSource in 'dataSources'.");
            hasErrors = true;
        }
        else {
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
                }
                else {
                    console.log(`  [OK] ${dsName}: contract address valid (${addr}).`);
                }
                // Check ABIs
                const abis = ds.mapping?.abis || [];
                if (!Array.isArray(abis) || abis.length === 0) {
                    console.error(`  [FAIL] ${dsName}: mapping.abis must list at least one ABI file.`);
                    hasErrors = true;
                }
                else {
                    for (const abiRef of abis) {
                        const abiFile = path.resolve(projectDir, abiRef.file);
                        if (!fs.existsSync(abiFile)) {
                            console.error(`  [FAIL] ${dsName}: ABI file not found at ${abiRef.file}`);
                            hasErrors = true;
                        }
                        else {
                            try {
                                const parsed = JSON.parse(fs.readFileSync(abiFile, "utf-8"));
                                if (!Array.isArray(parsed)) {
                                    console.error(`  [FAIL] ${dsName}: ABI file ${abiRef.file} is not a JSON array.`);
                                    hasErrors = true;
                                }
                                else {
                                    const eventCount = parsed.filter((item) => item.type === "event").length;
                                    console.log(`  [OK] ${dsName}: ABI ${abiRef.name} loaded with ${eventCount} event definition(s).`);
                                }
                            }
                            catch (e) {
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
                }
                else {
                    console.log(`  [OK] ${dsName}: handler entry point exists at src/mapping.ts`);
                }
                // Check resource limits if specified
                if (ds.mapping?.fuelLimit !== undefined) {
                    const fuel = ds.mapping.fuelLimit;
                    if (typeof fuel !== "number" || fuel < 1000) {
                        console.error(`  [FAIL] ${dsName}: mapping.fuelLimit must be a number >= 1000 (got ${fuel}).`);
                        hasErrors = true;
                    }
                    else {
                        console.log(`  [OK] ${dsName}: fuel limit set to ${fuel.toLocaleString()} ticks.`);
                    }
                }
                if (ds.mapping?.maxMemoryMb !== undefined) {
                    const mem = ds.mapping.maxMemoryMb;
                    if (typeof mem !== "number" || mem < 1 || mem > 512) {
                        console.error(`  [FAIL] ${dsName}: mapping.maxMemoryMb must be between 1 and 512 MB (got ${mem}).`);
                        hasErrors = true;
                    }
                    else {
                        console.log(`  [OK] ${dsName}: max memory ceiling set to ${mem} MB.`);
                    }
                }
            }
        }
        // 3. Check network fallback RPC URLs
        if (config.network?.fallbackRpcUrls) {
            if (!Array.isArray(config.network.fallbackRpcUrls)) {
                console.error("  [FAIL] network.fallbackRpcUrls must be a list of URL strings.");
                hasErrors = true;
            }
            else {
                for (const url of config.network.fallbackRpcUrls) {
                    if (!/^https?:\/\/.+/.test(url)) {
                        console.error(`  [FAIL] network.fallbackRpcUrls: invalid URL '${url}'. Must start with http:// or https://`);
                        hasErrors = true;
                    }
                }
                console.log(`  [OK] Configured ${config.network.fallbackRpcUrls.length} fallback RPC endpoint(s).`);
            }
        }
        // 4. Check webhooks
        if (config.webhooks) {
            if (!Array.isArray(config.webhooks)) {
                console.error("  [FAIL] webhooks must be an array of webhook objects.");
                hasErrors = true;
            }
            else {
                for (let i = 0; i < config.webhooks.length; i++) {
                    const wh = config.webhooks[i];
                    const whDesc = `webhook[${i}] (${wh.url || "unspecified"})`;
                    if (!wh.url || !/^https?:\/\/.+/.test(wh.url)) {
                        console.error(`  [FAIL] ${whDesc}: invalid URL. Must start with http:// or https://`);
                        hasErrors = true;
                    }
                    if (wh.maxRetries !== undefined && (typeof wh.maxRetries !== "number" || wh.maxRetries < 1)) {
                        console.error(`  [FAIL] ${whDesc}: maxRetries must be an integer >= 1.`);
                        hasErrors = true;
                    }
                }
                console.log(`  [OK] Configured ${config.webhooks.length} declarative webhook(s).`);
            }
        }
        // 5. Check indexing tuning
        if (config.indexing) {
            if (config.indexing.ringBufferDepth !== undefined) {
                const depth = config.indexing.ringBufferDepth;
                if (typeof depth !== "number" || depth < 10) {
                    console.error(`  [FAIL] indexing.ringBufferDepth must be >= 10 (got ${depth}).`);
                    hasErrors = true;
                }
                else {
                    console.log(`  [OK] Reorg ring buffer depth set to ${depth}.`);
                }
            }
            if (config.indexing.reconcilerIntervalSecs !== undefined) {
                const secs = config.indexing.reconcilerIntervalSecs;
                if (typeof secs !== "number" || secs < 1) {
                    console.error(`  [FAIL] indexing.reconcilerIntervalSecs must be >= 1 (got ${secs}).`);
                    hasErrors = true;
                }
                else {
                    console.log(`  [OK] Gap reconciler interval set to ${secs}s.`);
                }
            }
        }
        // 6. Check schema.graphql
        const schemaPath = path.join(projectDir, "schema.graphql");
        if (!fs.existsSync(schemaPath)) {
            console.error("  [FAIL] schema.graphql not found.");
            hasErrors = true;
        }
        else {
            const schemaContent = fs.readFileSync(schemaPath, "utf-8");
            const entityCount = (schemaContent.match(/type\s+\w+\s+@entity/g) || []).length;
            if (entityCount === 0) {
                console.error("  [FAIL] schema.graphql has no @entity types declared.");
                hasErrors = true;
            }
            else {
                console.log(`  [OK] schema.graphql valid with ${entityCount} @entity definition(s).`);
            }
        }
        console.log("");
        if (hasErrors) {
            console.error("Validation failed. Please address the errors above before building.\n");
            process.exit(1);
        }
        else {
            console.log("Validation passed successfully! All configuration files, ABIs, and schemas are valid.\n");
        }
    });
    return cmd;
}
