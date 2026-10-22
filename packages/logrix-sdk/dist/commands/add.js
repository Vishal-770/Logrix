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
exports.addCommand = addCommand;
const commander_1 = require("commander");
const prompts_1 = require("@inquirer/prompts");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const picocolors_1 = __importDefault(require("picocolors"));
const yaml_1 = require("yaml");
const templates_1 = require("../templates");
const ui_1 = require("../ui");
const errors_1 = require("../errors");
const EXPLORER_APIS = {
    1: "https://api.etherscan.io/api",
    42161: "https://api.arbiscan.io/api",
    10: "https://api-optimistic.etherscan.io/api",
    8453: "https://api.basescan.org/api",
    137: "https://api.polygonscan.com/api",
    56: "https://api.bscscan.com/api",
    43114: "https://api.snowtrace.io/api",
    59144: "https://api.lineascan.build/api",
    534352: "https://api.scrollscan.com/api",
    81457: "https://api.blastscan.io/api",
    100: "https://api.gnosisscan.io/api",
    250: "https://api.ftmscan.com/api",
    42220: "https://api.celoscan.io/api",
    1101: "https://api-zkevm.polygonscan.com/api",
    11155111: "https://api-sepolia.etherscan.io/api",
    17000: "https://api-holesky.etherscan.io/api",
    421614: "https://api-sepolia.arbiscan.io/api",
    84532: "https://api-sepolia.basescan.org/api",
    11155420: "https://api-sepolia-optimistic.etherscan.io/api",
    80002: "https://api-amoy.polygonscan.com/api",
    97: "https://api-testnet.bscscan.com/api",
    43113: "https://api-testnet.snowtrace.io/api",
    59141: "https://api-sepolia.lineascan.build/api",
    534351: "https://api-sepolia.scrollscan.com/api",
    168587773: "https://api-sepolia.blastscan.io/api",
};
/**
 * Fetch ABI from Etherscan/Etherscan-compatible chain explorer or Sourcify as fallback.
 * Supports all 26+ major EVM mainnets and testnets.
 */
async function fetchAbi(address, chainId = 1, apiKey) {
    const addr = address.toLowerCase();
    // Try Etherscan-compatible explorer API if registered for this chain
    const baseExplorer = EXPLORER_APIS[chainId];
    if (baseExplorer) {
        try {
            const keyParam = apiKey ? `&apikey=${apiKey}` : "";
            const url = `${baseExplorer}?module=contract&action=getabi&address=${addr}${keyParam}`;
            const resp = await fetch(url);
            const json = await resp.json();
            if (json.status === "1" && json.result) {
                const parsed = JSON.parse(json.result);
                if (Array.isArray(parsed) && parsed.length > 0) {
                    ui_1.ui.info("Fetched verified ABI from block explorer", address);
                    return parsed;
                }
            }
        }
        catch {
            // fall through to Sourcify
        }
    }
    // Try Sourcify (supports any chainId, no API key required)
    try {
        const url = `https://repo.sourcify.dev/contracts/full_match/${chainId}/${addr}/metadata.json`;
        const resp = await fetch(url);
        if (resp.ok) {
            const json = await resp.json();
            const abi = json?.output?.abi;
            if (Array.isArray(abi) && abi.length > 0) {
                ui_1.ui.info("Fetched verified ABI from Sourcify", address);
                return abi;
            }
        }
    }
    catch {
        // fall through
    }
    return null;
}
function addCommand() {
    const cmd = new commander_1.Command("add");
    cmd
        .description("Add components to an existing Logrix indexer project")
        .argument("<type>", "Type of component to add (contract or webhook)")
        .argument("[nameOrUrl]", "Name of contract or URL of webhook")
        .option("--address <address>", "Contract address")
        .option("--abi <path>", "Path to ABI JSON file")
        .option("--fetch-abi", "Auto-fetch ABI from block explorer / Sourcify")
        .option("--etherscan-key <key>", "Explorer API key for ABI fetch (optional)")
        .option("--start-block <block>", "Starting block number", "0")
        .option("--events <events>", "Comma-separated event names (e.g. reorg,Transfer)")
        .option("--secret <secret>", "Webhook signing secret (optional)")
        .option("--max-retries <retries>", "Maximum webhook delivery retries", "3")
        .action(async (type, nameArg, opts) => {
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");
        if (!fs.existsSync(configPath)) {
            throw new errors_1.CliError("logrix.yaml not found", "Project manifest is missing in current directory", "Run this command from your indexer project root directory");
        }
        if (type !== "contract" && type !== "webhook") {
            throw new errors_1.CliError(`Unknown component type '${type}'`, "Supported component types: 'contract', 'webhook'", "Example: logrix add contract Token --address 0x... or logrix add webhook https://...");
        }
        let config;
        try {
            config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
        }
        catch (e) {
            throw new errors_1.CliError("Failed to parse logrix.yaml", e.message);
        }
        if (type === "webhook") {
            ui_1.ui.banner("Add Webhook", "Registering event destination");
            let url = nameArg;
            if (!url) {
                if (process.stdin.isTTY) {
                    url = await (0, prompts_1.input)({
                        message: "Webhook Destination URL:",
                        validate: (v) => /^https?:\/\/.+/.test(v.trim()) || "Must be a valid http:// or https:// URL",
                    });
                }
                else {
                    ui_1.ui.error("Webhook URL required", "Usage: logrix add webhook <url>");
                    process.exit(1);
                }
            }
            url = url.trim();
            if (!/^https?:\/\/.+/.test(url)) {
                ui_1.ui.error("Invalid webhook URL", "URL must start with http:// or https://");
                process.exit(1);
            }
            const events = opts.events
                ? opts.events.split(",").map((s) => s.trim()).filter(Boolean)
                : ["reorg"];
            const maxRetries = parseInt(opts.maxRetries || "3", 10);
            if (!config.webhooks) {
                config.webhooks = [];
            }
            const webhookEntry = {
                url,
                events,
                maxRetries,
            };
            if (opts.secret) {
                webhookEntry.secret = opts.secret.trim();
            }
            config.webhooks.push(webhookEntry);
            fs.writeFileSync(configPath, (0, yaml_1.stringify)(config, { indent: 2 }));
            ui_1.ui.success("Webhook registered in logrix.yaml", url);
            ui_1.ui.label("Events", events.join(", "));
            ui_1.ui.label("Max Retries", String(maxRetries));
            if (opts.secret) {
                ui_1.ui.label("Signing Secret", "*** (configured)");
            }
            console.log();
            return;
        }
        ui_1.ui.banner("Add Contract", "Registering contract data source");
        if (!config.dataSources) {
            config.dataSources = [];
        }
        const chainId = config.network?.chainId ?? 1;
        // 1. Resolve contract name
        let contractName = nameArg;
        if (!contractName) {
            if (process.stdin.isTTY) {
                contractName = await (0, prompts_1.input)({
                    message: "Smart Contract Name:",
                    default: "SecondaryContract",
                    validate: (v) => /^[A-Za-z][A-Za-z0-9]*$/.test(v.trim()) ||
                        "Must be alphanumeric, starting with a letter",
                });
            }
            else {
                contractName = "SecondaryContract";
            }
        }
        contractName = contractName.trim();
        // Check for collision
        const exists = config.dataSources.some((ds) => ds.name === contractName);
        if (exists) {
            ui_1.ui.error(`Contract '${contractName}' already exists`, "Duplicate contract names are not permitted in logrix.yaml.");
            process.exit(1);
        }
        // 2. Resolve contract address
        let contractAddress = opts.address;
        if (!contractAddress) {
            if (process.stdin.isTTY) {
                contractAddress = await (0, prompts_1.input)({
                    message: "Target Contract Address:",
                    default: "0x0000000000000000000000000000000000000000",
                    validate: (v) => /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x hex address",
                });
            }
            else {
                contractAddress = "0x0000000000000000000000000000000000000000";
            }
        }
        contractAddress = contractAddress.trim();
        // 3. Resolve start block
        let startBlockStr = opts.startBlock;
        if (!opts.address && process.stdin.isTTY) {
            startBlockStr = await (0, prompts_1.input)({
                message: "Start Block:",
                default: "0",
                validate: (v) => /^\d+$/.test(v.trim()) || "Must be a non-negative integer",
            });
        }
        const startBlock = parseInt(startBlockStr.trim(), 10);
        // 4. Resolve ABI
        const abisDir = path.join(projectDir, "abis");
        fs.mkdirSync(abisDir, { recursive: true });
        const targetAbiFile = path.join(abisDir, `${contractName}.json`);
        if (opts.abi) {
            const sourceAbi = path.resolve(projectDir, opts.abi);
            if (!fs.existsSync(sourceAbi)) {
                ui_1.ui.error(`ABI file not found: ${opts.abi}`);
                process.exit(1);
            }
            fs.copyFileSync(sourceAbi, targetAbiFile);
            ui_1.ui.success("Imported ABI file", `abis/${contractName}.json`);
        }
        else if (opts.fetchAbi) {
            ui_1.ui.info(`Fetching verified ABI for ${contractAddress} (Chain ID: ${chainId}) ...`);
            const abi = await fetchAbi(contractAddress, chainId, opts.etherscanKey);
            if (abi) {
                fs.writeFileSync(targetAbiFile, JSON.stringify(abi, null, 2));
                ui_1.ui.success("Verified ABI downloaded", `abis/${contractName}.json`);
            }
            else {
                throw new errors_1.CliError(`Could not fetch verified ABI for ${contractAddress} on chain ${chainId}`, "Neither block explorer nor Sourcify returned a verified ABI for this contract.", "Provide the ABI file manually with --abi <path> or verify the contract source code.");
            }
        }
        else if (!fs.existsSync(targetAbiFile)) {
            fs.writeFileSync(targetAbiFile, JSON.stringify(templates_1.ERC20_ABI, null, 2));
            ui_1.ui.info("Created ERC-20 placeholder ABI", `abis/${contractName}.json`);
        }
        // 5. Append new dataSource to logrix.yaml
        const networkName = config.network?.name || "mainnet";
        const newDataSource = {
            kind: "ethereum/contract",
            name: contractName,
            network: networkName,
            source: {
                address: contractAddress,
                abi: contractName,
                startBlock: startBlock,
            },
            mapping: {
                kind: "wasm/assemblyscript",
                file: "./build/mapping.wasm",
                abis: [
                    {
                        name: contractName,
                        file: `./abis/${contractName}.json`,
                    },
                ],
                eventHandlers: [
                    {
                        event: "Transfer(address indexed,address indexed,uint256)",
                        handler: `handle${contractName}Transfer`,
                    },
                ],
            },
        };
        config.dataSources.push(newDataSource);
        fs.writeFileSync(configPath, (0, yaml_1.stringify)(config, { indent: 2 }));
        ui_1.ui.success(`Contract '${contractName}' added to logrix.yaml`);
        ui_1.ui.label("Contract Name", contractName);
        ui_1.ui.label("Address", contractAddress);
        ui_1.ui.label("Start Block", String(startBlock));
        ui_1.ui.label("Network", `${networkName} (${chainId})`);
        ui_1.ui.section("Next Steps");
        console.log(`  1. Review ABI at ${picocolors_1.default.cyan(`abis/${contractName}.json`)}`);
        console.log(`  2. Generate type bindings: ${picocolors_1.default.bold("logrix codegen")}`);
        console.log(`  3. Compile AssemblyScript handler: ${picocolors_1.default.bold("logrix build")}`);
        console.log();
    });
    return cmd;
}
