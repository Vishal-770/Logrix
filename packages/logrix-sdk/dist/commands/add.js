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
exports.addCommand = addCommand;
const commander_1 = require("commander");
const prompts_1 = require("@inquirer/prompts");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const yaml_1 = require("yaml");
const templates_1 = require("../templates");
function addCommand() {
    const cmd = new commander_1.Command("add");
    cmd
        .description("Add components to an existing Logrix indexer project")
        .argument("<type>", "Type of component to add (e.g. contract)")
        .argument("[name]", "Name of the component")
        .option("--address <address>", "Contract address")
        .option("--abi <path>", "Path to ABI JSON file")
        .option("--start-block <block>", "Starting block number", "0")
        .action(async (type, nameArg, opts) => {
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");
        if (!fs.existsSync(configPath)) {
            console.error("Error: logrix.yaml not found. Please run this command from the project root.");
            process.exit(1);
        }
        if (type !== "contract") {
            console.error(`Error: Unknown component type '${type}'. Supported types: contract`);
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
        if (!config.dataSources) {
            config.dataSources = [];
        }
        // 1. Resolve contract name
        let contractName = nameArg;
        if (!contractName) {
            if (process.stdin.isTTY) {
                contractName = await (0, prompts_1.input)({
                    message: "Smart Contract Name:",
                    default: "SecondaryContract",
                    validate: (v) => /^[A-Za-z][A-Za-z0-9]*$/.test(v.trim()) || "Must be alphanumeric, starting with a letter",
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
            console.error(`Error: A contract named '${contractName}' is already defined in logrix.yaml.`);
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
        // 4. Resolve ABI file
        const abisDir = path.join(projectDir, "abis");
        fs.mkdirSync(abisDir, { recursive: true });
        const targetAbiFile = path.join(abisDir, `${contractName}.json`);
        if (opts.abi) {
            const sourceAbi = path.resolve(projectDir, opts.abi);
            if (!fs.existsSync(sourceAbi)) {
                console.error(`Error: Specified ABI file not found: ${opts.abi}`);
                process.exit(1);
            }
            fs.copyFileSync(sourceAbi, targetAbiFile);
            console.log(`  Copied ABI from ${opts.abi} to abis/${contractName}.json`);
        }
        else if (!fs.existsSync(targetAbiFile)) {
            fs.writeFileSync(targetAbiFile, JSON.stringify(templates_1.ERC20_ABI, null, 2));
            console.log(`  Created default ERC-20 placeholder ABI at abis/${contractName}.json`);
        }
        // 5. Append new dataSource
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
        console.log(`\nSuccessfully added contract '${contractName}' to logrix.yaml!`);
        console.log("Next steps:");
        console.log(`  1. Place your real ABI at: abis/${contractName}.json`);
        console.log("  2. Run: logrix codegen");
        console.log("  3. Run: logrix build\n");
    });
    return cmd;
}
