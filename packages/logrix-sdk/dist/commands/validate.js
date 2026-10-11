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
const ui_1 = require("../ui");
const schema_1 = require("../schema");
const errors_1 = require("../errors");
function validateCommand() {
    const cmd = new commander_1.Command("validate");
    cmd
        .description("Validate project configuration, ABIs, schema, and handler files")
        .action(() => {
        const projectDir = process.cwd();
        let totalChecks = 0;
        let passedChecks = 0;
        let hasErrors = false;
        ui_1.ui.banner("Project Validation", "Verifying manifest, ABIs, schema, and handlers");
        // 1. Check logrix.yaml
        ui_1.ui.section("Configuration Manifest");
        const configPath = path.join(projectDir, "logrix.yaml");
        totalChecks++;
        if (!fs.existsSync(configPath)) {
            throw new errors_1.CliError("Manifest missing: logrix.yaml not found", "Project manifest is missing in current working directory", "Run `logrix init` to scaffold a new project");
        }
        else {
            passedChecks++;
        }
        let config;
        totalChecks++;
        try {
            config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
            ui_1.ui.success("Manifest syntax", "logrix.yaml is valid YAML");
            passedChecks++;
        }
        catch (e) {
            throw new errors_1.CliError("Manifest syntax error", e.message);
        }
        totalChecks++;
        const zodResult = schema_1.LogrixConfigSchema.safeParse(config);
        if (!zodResult.success) {
            const issues = zodResult.error.issues
                .map((i) => `${i.path.join(".")}: ${i.message}`)
                .join(", ");
            ui_1.ui.error("Zod Schema Validation Failed", issues);
            hasErrors = true;
        }
        else {
            ui_1.ui.success("Schema definition valid", "Manifest matches typed Logrix specification");
            passedChecks++;
        }
        totalChecks++;
        if (!config.dataSources || !Array.isArray(config.dataSources) || config.dataSources.length === 0) {
            ui_1.ui.error("Missing data sources", "logrix.yaml must declare at least one dataSource");
            hasErrors = true;
        }
        else {
            ui_1.ui.success("Data sources configured", `${config.dataSources.length} contract source(s) declared`);
            passedChecks++;
        }
        // 2. Check each dataSource
        if (Array.isArray(config.dataSources)) {
            ui_1.ui.section("Contract Sources & ABIs");
            for (let i = 0; i < config.dataSources.length; i++) {
                const ds = config.dataSources[i];
                const dsName = ds.name || `dataSource[${i}]`;
                totalChecks++;
                if (!ds.name) {
                    ui_1.ui.error(`Source [${i}] unnamed`, "Each dataSource must specify a name");
                    hasErrors = true;
                }
                else {
                    passedChecks++;
                }
                totalChecks++;
                const addr = ds.source?.address;
                if (!addr || !/^0x[0-9a-fA-F]{40}$/.test(addr)) {
                    ui_1.ui.error(`${dsName}: Invalid contract address`, `'${addr}' is not a valid 40-character 0x hex address`);
                    hasErrors = true;
                }
                else {
                    ui_1.ui.success(`${dsName}: Contract address`, addr);
                    passedChecks++;
                }
                // Check ABIs
                const abis = ds.mapping?.abis || [];
                totalChecks++;
                if (!Array.isArray(abis) || abis.length === 0) {
                    ui_1.ui.error(`${dsName}: Missing ABI reference`, "mapping.abis must specify at least one ABI JSON file");
                    hasErrors = true;
                }
                else {
                    for (const abiRef of abis) {
                        const abiFile = path.resolve(projectDir, abiRef.file);
                        totalChecks++;
                        if (!fs.existsSync(abiFile)) {
                            ui_1.ui.error(`${dsName}: ABI file missing`, `File not found at ${abiRef.file}`);
                            hasErrors = true;
                        }
                        else {
                            try {
                                const parsed = JSON.parse(fs.readFileSync(abiFile, "utf-8"));
                                if (!Array.isArray(parsed)) {
                                    ui_1.ui.error(`${dsName}: Invalid ABI format`, `${abiRef.file} is not a valid JSON array`);
                                    hasErrors = true;
                                }
                                else {
                                    const eventCount = parsed.filter((item) => item.type === "event").length;
                                    ui_1.ui.success(`${dsName}: ABI ${abiRef.name}`, `${eventCount} event definition(s) loaded`);
                                    passedChecks++;
                                }
                            }
                            catch (e) {
                                ui_1.ui.error(`${dsName}: ABI parsing failed`, `${abiRef.file}: ${e.message}`);
                                hasErrors = true;
                            }
                        }
                    }
                }
                // Check handler mapping file
                totalChecks++;
                const mappingSrc = path.resolve(projectDir, "src/mapping.ts");
                if (!fs.existsSync(mappingSrc)) {
                    ui_1.ui.error(`${dsName}: Handler missing`, "Entry point not found at src/mapping.ts");
                    hasErrors = true;
                }
                else {
                    ui_1.ui.success(`${dsName}: Handler entry`, "src/mapping.ts exists");
                    passedChecks++;
                }
                // Check resource limits if specified
                if (ds.mapping?.fuelLimit !== undefined) {
                    totalChecks++;
                    const fuel = ds.mapping.fuelLimit;
                    if (typeof fuel !== "number" || fuel < 1000) {
                        ui_1.ui.error(`${dsName}: Invalid fuel limit`, `mapping.fuelLimit must be a number >= 1000 (got ${fuel})`);
                        hasErrors = true;
                    }
                    else {
                        ui_1.ui.success(`${dsName}: Fuel limit`, `${fuel.toLocaleString()} ticks`);
                        passedChecks++;
                    }
                }
                if (ds.mapping?.maxMemoryMb !== undefined) {
                    totalChecks++;
                    const mem = ds.mapping.maxMemoryMb;
                    if (typeof mem !== "number" || mem < 1 || mem > 512) {
                        ui_1.ui.error(`${dsName}: Invalid memory ceiling`, `mapping.maxMemoryMb must be between 1 and 512 MB (got ${mem})`);
                        hasErrors = true;
                    }
                    else {
                        ui_1.ui.success(`${dsName}: Memory ceiling`, `${mem} MB`);
                        passedChecks++;
                    }
                }
            }
        }
        // 3. Check network fallback RPC URLs
        if (config.network?.fallbackRpcUrls) {
            ui_1.ui.section("Network & Failover");
            totalChecks++;
            if (!Array.isArray(config.network.fallbackRpcUrls)) {
                ui_1.ui.error("Invalid fallbackRpcUrls", "Must be a list of URL strings");
                hasErrors = true;
            }
            else {
                for (const url of config.network.fallbackRpcUrls) {
                    totalChecks++;
                    if (!/^https?:\/\/.+/.test(url)) {
                        ui_1.ui.error("Invalid fallback URL", `'${url}' must start with http:// or https://`);
                        hasErrors = true;
                    }
                    else {
                        passedChecks++;
                    }
                }
                ui_1.ui.success("Fallback RPC endpoints", `${config.network.fallbackRpcUrls.length} URL(s) configured`);
                passedChecks++;
            }
        }
        // 4. Check webhooks
        if (config.webhooks) {
            ui_1.ui.section("Declarative Webhooks");
            totalChecks++;
            if (!Array.isArray(config.webhooks)) {
                ui_1.ui.error("Invalid webhooks format", "Must be an array of webhook objects");
                hasErrors = true;
            }
            else {
                for (let i = 0; i < config.webhooks.length; i++) {
                    const wh = config.webhooks[i];
                    totalChecks++;
                    if (!wh.url || !/^https?:\/\/.+/.test(wh.url)) {
                        ui_1.ui.error(`Webhook [${i}] invalid URL`, "Must start with http:// or https://");
                        hasErrors = true;
                    }
                    else {
                        passedChecks++;
                    }
                }
                ui_1.ui.success("Webhooks verified", `${config.webhooks.length} subscriber endpoint(s) declared`);
                passedChecks++;
            }
        }
        // 5. Check schema.graphql
        ui_1.ui.section("GraphQL Schema");
        const schemaPath = path.join(projectDir, "schema.graphql");
        totalChecks++;
        if (!fs.existsSync(schemaPath)) {
            ui_1.ui.error("Schema file missing", "schema.graphql not found in project root");
            hasErrors = true;
        }
        else {
            const schemaContent = fs.readFileSync(schemaPath, "utf-8");
            const entityCount = (schemaContent.match(/type\s+\w+\s+@entity/g) || []).length;
            if (entityCount === 0) {
                ui_1.ui.error("No entities defined", "schema.graphql must declare at least one type with @entity");
                hasErrors = true;
            }
            else {
                ui_1.ui.success("Schema valid", `${entityCount} @entity definition(s) declared`);
                passedChecks++;
            }
        }
        ui_1.ui.divider();
        console.log();
        if (hasErrors) {
            ui_1.ui.error("Validation Failed", `Passed ${passedChecks} of ${totalChecks} checks. Address the errors above before compiling.`);
            console.log();
            process.exit(1);
        }
        else {
            ui_1.ui.success("Validation Passed", `All ${passedChecks} checks completed successfully. Ready for compilation.`);
            console.log();
        }
    });
    return cmd;
}
