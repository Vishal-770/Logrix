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
exports.NETWORKS = void 0;
exports.initCommand = initCommand;
const commander_1 = require("commander");
const prompts_1 = require("@inquirer/prompts");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const templates_1 = require("../templates");
exports.NETWORKS = {
    // Mainnets
    "ethereum": { name: "Ethereum Mainnet", chainId: 1, rpcUrl: "https://eth.llamarpc.com", category: "mainnet" },
    "arbitrum-one": { name: "Arbitrum One", chainId: 42161, rpcUrl: "https://arb1.arbitrum.io/rpc", category: "mainnet" },
    "base": { name: "Base Mainnet", chainId: 8453, rpcUrl: "https://mainnet.base.org", category: "mainnet" },
    "optimism": { name: "Optimism Mainnet", chainId: 10, rpcUrl: "https://mainnet.optimism.io", category: "mainnet" },
    "polygon": { name: "Polygon PoS", chainId: 137, rpcUrl: "https://polygon-rpc.com", category: "mainnet" },
    "bsc": { name: "BNB Smart Chain", chainId: 56, rpcUrl: "https://binance.llamarpc.com", category: "mainnet" },
    "avalanche": { name: "Avalanche C-Chain", chainId: 43114, rpcUrl: "https://api.avax.network/ext/bc/C/rpc", category: "mainnet" },
    "linea": { name: "Linea Mainnet", chainId: 59144, rpcUrl: "https://rpc.linea.build", category: "mainnet" },
    "scroll": { name: "Scroll Mainnet", chainId: 534352, rpcUrl: "https://rpc.scroll.io", category: "mainnet" },
    "blast": { name: "Blast Mainnet", chainId: 81457, rpcUrl: "https://rpc.blast.io", category: "mainnet" },
    "zksync": { name: "ZKsync Era", chainId: 324, rpcUrl: "https://mainnet.era.zksync.io", category: "mainnet" },
    "gnosis": { name: "Gnosis Chain", chainId: 100, rpcUrl: "https://rpc.gnosischain.com", category: "mainnet" },
    "fantom": { name: "Fantom Opera", chainId: 250, rpcUrl: "https://rpcapi.fantom.network", category: "mainnet" },
    "celo": { name: "Celo Mainnet", chainId: 42220, rpcUrl: "https://forno.celo.org", category: "mainnet" },
    "polygon-zkevm": { name: "Polygon zkEVM", chainId: 1101, rpcUrl: "https://zkevm-rpc.com", category: "mainnet" },
    // Testnets
    "sepolia": { name: "Ethereum Sepolia", chainId: 11155111, rpcUrl: "https://rpc.sepolia.org", category: "testnet" },
    "holesky": { name: "Ethereum Holesky", chainId: 17000, rpcUrl: "https://ethereum-holesky-rpc.publicnode.com", category: "testnet" },
    "arbitrum-sepolia": { name: "Arbitrum Sepolia", chainId: 421614, rpcUrl: "https://sepolia-rollup.arbitrum.io/rpc", category: "testnet" },
    "base-sepolia": { name: "Base Sepolia", chainId: 84532, rpcUrl: "https://sepolia.base.org", category: "testnet" },
    "optimism-sepolia": { name: "Optimism Sepolia", chainId: 11155420, rpcUrl: "https://sepolia.optimism.io", category: "testnet" },
    "polygon-amoy": { name: "Polygon Amoy", chainId: 80002, rpcUrl: "https://rpc-amoy.polygon.technology", category: "testnet" },
    "bsc-testnet": { name: "BNB Smart Chain Testnet", chainId: 97, rpcUrl: "https://data-seed-prebsc-1-s1.binance.org:8545", category: "testnet" },
    "avalanche-fuji": { name: "Avalanche Fuji", chainId: 43113, rpcUrl: "https://api.avax-test.network/ext/bc/C/rpc", category: "testnet" },
    "linea-sepolia": { name: "Linea Sepolia", chainId: 59141, rpcUrl: "https://rpc.sepolia.linea.build", category: "testnet" },
    "scroll-sepolia": { name: "Scroll Sepolia", chainId: 534351, rpcUrl: "https://sepolia-rpc.scroll.io", category: "testnet" },
    "blast-sepolia": { name: "Blast Sepolia", chainId: 168587773, rpcUrl: "https://sepolia.blast.io", category: "testnet" },
};
function initCommand() {
    const cmd = new commander_1.Command("init");
    cmd
        .description("Scaffold a new Logrix indexer project")
        .argument("[name]", "Project directory name")
        .option("-y, --yes", "Skip prompts and use defaults (non-interactive)")
        .option("--network <network>", "Network preset name (e.g. arbitrum-one, ethereum, base, sepolia)")
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
        if (!networkKey || !exports.NETWORKS[networkKey]) {
            if (isNonInteractive) {
                networkKey = "arbitrum-one";
            }
            else {
                networkKey = await (0, prompts_1.select)({
                    message: "Network:",
                    choices: Object.entries(exports.NETWORKS).map(([key, net]) => ({
                        name: `[${net.category.toUpperCase()}] ${net.name} (Chain ID: ${net.chainId})`,
                        value: key,
                    })),
                    default: "arbitrum-one",
                });
            }
        }
        const network = exports.NETWORKS[networkKey] || exports.NETWORKS["arbitrum-one"];
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
                    validate: (v) => /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x hex address",
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
                    message: "Start block (indexing starting point):",
                    default: "0",
                    validate: (v) => /^\d+$/.test(v.trim()) || "Must be a non-negative integer",
                });
            }
        }
        const startBlock = parseInt(startBlockStr.trim(), 10);
        const answers = {
            projectName,
            networkName: networkKey,
            chainId: network.chainId,
            rpcUrl,
            contractName,
            contractAddress,
            startBlock,
        };
        console.log(`\nScaffolding in ./${projectName}/ ...\n`);
        // Create directories
        fs.mkdirSync(path.join(targetDir, "abis"), { recursive: true });
        fs.mkdirSync(path.join(targetDir, "src"), { recursive: true });
        fs.mkdirSync(path.join(targetDir, "deploy"), { recursive: true });
        // Write core indexer files
        fs.writeFileSync(path.join(targetDir, "logrix.yaml"), (0, templates_1.logrixYamlTemplate)(answers));
        fs.writeFileSync(path.join(targetDir, "schema.graphql"), (0, templates_1.schemaGraphqlTemplate)());
        fs.writeFileSync(path.join(targetDir, "abis", `${contractName}.json`), JSON.stringify(templates_1.ERC20_ABI, null, 2));
        fs.writeFileSync(path.join(targetDir, "src", "mapping.ts"), (0, templates_1.mappingTemplate)(contractName));
        fs.writeFileSync(path.join(targetDir, "package.json"), JSON.stringify((0, templates_1.packageJsonTemplate)(projectName), null, 2));
        fs.writeFileSync(path.join(targetDir, "tsconfig.json"), JSON.stringify((0, templates_1.tsconfigTemplate)(), null, 2));
        fs.writeFileSync(path.join(targetDir, ".gitignore"), (0, templates_1.gitignoreTemplate)());
        fs.writeFileSync(path.join(targetDir, "README.md"), (0, templates_1.readmeTemplate)(projectName));
        // Write deployment configuration profiles
        fs.writeFileSync(path.join(targetDir, "deploy", "values-local.yaml"), (0, templates_1.valuesLocalTemplate)(answers));
        fs.writeFileSync(path.join(targetDir, "deploy", "values-aws.yaml"), (0, templates_1.valuesAwsTemplate)(answers));
        fs.writeFileSync(path.join(targetDir, "deploy", "secrets.example.yaml"), (0, templates_1.secretsExampleTemplate)(answers));
        console.log("Done. Next steps:\n");
        console.log(`  cd ${projectName}`);
        console.log("  npm install");
        console.log("  npm run codegen");
        console.log("  npm run validate");
        console.log("  npm run build");
        console.log("  npm run export-values\n");
        console.log("Deploy with Helm:\n");
        console.log(`  Local: helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-local.yaml -f indexer-values.yaml`);
        console.log(`  AWS:   helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-aws.yaml -f indexer-values.yaml\n`);
    });
    return cmd;
}
