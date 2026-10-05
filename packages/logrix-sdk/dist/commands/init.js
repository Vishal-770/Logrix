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
exports.initCommand = initCommand;
const commander_1 = require("commander");
const prompts_1 = require("@inquirer/prompts");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const templates_1 = require("../templates");
const NETWORKS = {
    "ethereum": { name: "ethereum", chainId: 1, rpcUrl: "https://eth.llamarpc.com" },
    "arbitrum-one": { name: "arbitrum-one", chainId: 42161, rpcUrl: "https://arb1.arbitrum.io/rpc" },
    "base": { name: "base", chainId: 8453, rpcUrl: "https://mainnet.base.org" },
    "polygon": { name: "polygon", chainId: 137, rpcUrl: "https://polygon-rpc.com" },
    "arbitrum-sepolia": { name: "arbitrum-sepolia", chainId: 421614, rpcUrl: "https://sepolia-rollup.arbitrum.io/rpc" },
    "sepolia": { name: "sepolia", chainId: 11155111, rpcUrl: "https://rpc.sepolia.org" },
};
function initCommand() {
    const cmd = new commander_1.Command("init");
    cmd
        .description("Scaffold a new Logrix indexer project")
        .argument("[name]", "Project directory name")
        .option("-y, --yes", "Skip prompts and use defaults (non-interactive)")
        .option("--network <network>", "Network preset name (e.g. arbitrum-one, ethereum, base)")
        .option("--rpc <url>", "Custom RPC endpoint URL")
        .option("--contract-name <name>", "Contract name")
        .option("--address <address>", "Contract target address")
        .option("--start-block <block>", "Starting block number")
        .action(async (nameArg, opts) => {
        console.log("\nLogrix Project Initializer\n");
        const isNonInteractive = opts.yes || !process.stdin.isTTY;
        // Project Name
        let projectName = nameArg || opts.name;
        if (!projectName) {
            if (isNonInteractive) {
                projectName = "my-indexer";
            }
            else {
                projectName = await (0, prompts_1.input)({
                    message: "Project name:",
                    default: "my-indexer",
                    validate: (v) => v.trim().length > 0 || "Name cannot be empty",
                });
            }
        }
        projectName = projectName.trim();
        const targetDir = path.resolve(process.cwd(), projectName);
        if (fs.existsSync(targetDir)) {
            console.error(`Error: directory '${projectName}' already exists.`);
            process.exit(1);
        }
        // Network selection
        let networkKey = opts.network;
        if (!networkKey || !NETWORKS[networkKey]) {
            if (isNonInteractive) {
                networkKey = "arbitrum-one";
            }
            else {
                networkKey = await (0, prompts_1.select)({
                    message: "Network:",
                    choices: Object.entries(NETWORKS).map(([key, net]) => ({
                        name: `${net.name} (chain ${net.chainId})`,
                        value: key,
                    })),
                    default: "arbitrum-one",
                });
            }
        }
        const network = NETWORKS[networkKey] || NETWORKS["arbitrum-one"];
        // RPC URL
        let rpcUrl = opts.rpc;
        if (!rpcUrl) {
            if (isNonInteractive) {
                rpcUrl = network.rpcUrl;
            }
            else {
                rpcUrl = await (0, prompts_1.input)({
                    message: "RPC URL:",
                    default: network.rpcUrl,
                    validate: (v) => v.trim().length > 0 || "RPC URL cannot be empty",
                });
            }
        }
        rpcUrl = rpcUrl.trim();
        // Contract Name
        let contractName = opts.contractName;
        if (!contractName) {
            if (isNonInteractive) {
                contractName = "TokenContract";
            }
            else {
                contractName = await (0, prompts_1.input)({
                    message: "Contract name (used for ABI file and YAML):",
                    default: "TokenContract",
                    validate: (v) => /^[A-Za-z][A-Za-z0-9]*$/.test(v.trim()) ||
                        "Must be alphanumeric, starting with a letter",
                });
            }
        }
        contractName = contractName.trim();
        // Contract Address
        let contractAddress = opts.address;
        if (!contractAddress) {
            if (isNonInteractive) {
                contractAddress = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831";
            }
            else {
                contractAddress = await (0, prompts_1.input)({
                    message: "Contract address:",
                    default: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
                    validate: (v) => /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x address",
                });
            }
        }
        contractAddress = contractAddress.trim();
        // Start Block
        let startBlockStr = opts.startBlock;
        if (!startBlockStr) {
            if (isNonInteractive) {
                startBlockStr = "0";
            }
            else {
                startBlockStr = await (0, prompts_1.input)({
                    message: "Start block:",
                    default: "0",
                    validate: (v) => /^\d+$/.test(v.trim()) || "Must be a non-negative integer",
                });
            }
        }
        const answers = {
            projectName,
            networkName: network.name,
            chainId: network.chainId,
            rpcUrl,
            contractName,
            contractAddress,
            startBlock: parseInt(startBlockStr.trim(), 10),
        };
        console.log(`Scaffolding in ./${projectName}/ ...\n`);
        // Directory structure
        fs.mkdirSync(path.join(targetDir, "abis"), { recursive: true });
        fs.mkdirSync(path.join(targetDir, "src"), { recursive: true });
        fs.mkdirSync(path.join(targetDir, "build"), { recursive: true });
        // logrix.yaml
        fs.writeFileSync(path.join(targetDir, "logrix.yaml"), (0, templates_1.logrixYamlTemplate)(answers));
        // schema.graphql
        fs.writeFileSync(path.join(targetDir, "schema.graphql"), (0, templates_1.schemaGraphqlTemplate)());
        // Placeholder ABI
        fs.writeFileSync(path.join(targetDir, "abis", `${answers.contractName}.json`), JSON.stringify(templates_1.ERC20_ABI, null, 2));
        // src/mapping.ts
        fs.writeFileSync(path.join(targetDir, "src", "mapping.ts"), (0, templates_1.mappingTemplate)(answers.contractName));
        // package.json
        fs.writeFileSync(path.join(targetDir, "package.json"), JSON.stringify((0, templates_1.packageJsonTemplate)(projectName), null, 2));
        // tsconfig.json (AS config)
        fs.writeFileSync(path.join(targetDir, "tsconfig.json"), JSON.stringify((0, templates_1.tsconfigTemplate)(), null, 2));
        // .gitignore
        fs.writeFileSync(path.join(targetDir, ".gitignore"), (0, templates_1.gitignoreTemplate)());
        // README.md
        fs.writeFileSync(path.join(targetDir, "README.md"), (0, templates_1.readmeTemplate)(projectName));
        console.log("Done. Next steps:\n");
        console.log(`  cd ${projectName}`);
        console.log("  npm install");
        console.log("  npm run codegen");
        console.log("  npm run validate");
        console.log("  npm run build\n");
    });
    return cmd;
}
