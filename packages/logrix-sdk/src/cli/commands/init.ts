import { Command } from "commander";
import { input, select } from "@inquirer/prompts";
import * as fs from "fs";
import * as path from "path";
import pc from "picocolors";
import { ui } from "../ui";
import { CliError } from "../errors";
import {
  InitAnswers,
  logrixYamlTemplate,
  schemaGraphqlTemplate,
  mappingTemplate,
  packageJsonTemplate,
  tsconfigTemplate,
  gitignoreTemplate,
  readmeTemplate,
  valuesAwsTemplate,
  secretsExampleTemplate,
  ERC20_ABI,
} from "../templates";

export interface NetworkOption {
  name: string;
  chainId: number;
  rpcUrl: string;
  category: "mainnet" | "testnet";
}

export const NETWORKS: Record<string, NetworkOption> = {
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

export function initCommand(): Command {
  const cmd = new Command("init");

  cmd
    .description("Scaffold a new Logrix indexer project")
    .argument("[name]", "Project directory name")
    .option("-y, --yes", "Skip prompts and use defaults (non-interactive)")
    .option("--network <network>", "Network preset name (e.g. arbitrum-one, ethereum, base, sepolia)")
    .option("--rpc <url>", "Custom RPC endpoint URL")
    .option("--contract-name <name>", "Contract name")
    .option("--address <address>", "Contract target address")
    .option("--start-block <block>", "Starting block number")
    .option("--fallback-rpc <urls...>", "Fallback RPC endpoint URL(s)")
    .option("--webhook-url <url>", "Declarative webhook subscriber URL")
    .option("--reorg-depth <depth>", "Reorg ring buffer depth (default: 128)")
    .option("--reconciler-interval <secs>", "Gap reconciler interval in seconds (default: 30)")
    .action(async (nameArg: string | undefined, opts: any) => {
      ui.banner("Project Scaffolding", "Scaffold a production-ready Logrix indexer");

      const isNonInteractive = opts.yes || !process.stdin.isTTY;

      // Project Name
      let projectName = nameArg || opts.name;
      if (!projectName) {
        if (isNonInteractive) {
          projectName = "my-indexer";
        } else {
          projectName = await input({
            message: "Project name:",
            default: "my-indexer",
            validate: (v) => v.trim().length > 0 || "Name cannot be empty",
          });
        }
      }
      projectName = projectName.trim();

      const targetDir = path.resolve(process.cwd(), projectName);
      if (fs.existsSync(targetDir)) {
        throw new CliError(
          `Directory already exists: '${projectName}'`,
          `Destination path ${targetDir} already contains files`,
          "Choose a different project name or remove the existing directory"
        );
      }

      // Network selection
      if (opts.network && !NETWORKS[opts.network]) {
        const supported = Object.keys(NETWORKS).join(", ");
        throw new CliError(
          `Unknown network: '${opts.network}'`,
          `Supported networks are: ${supported}`,
          "Specify one of the supported network names with --network"
        );
      }

      let networkKey = opts.network;
      if (!networkKey) {
        if (isNonInteractive) {
          networkKey = "arbitrum-one";
        } else {
          networkKey = await select<string>({
            message: "Network:",
            choices: Object.entries(NETWORKS).map(([key, net]) => ({
              name: `[${net.category.toUpperCase()}] ${net.name} (Chain ID: ${net.chainId})`,
              value: key,
            })),
            default: "arbitrum-one",
          });
        }
      }

      const network = NETWORKS[networkKey];

      // RPC URL
      let rpcUrl = opts.rpc;
      if (!rpcUrl) {
        if (isNonInteractive) {
          rpcUrl = network.rpcUrl;
        } else {
          rpcUrl = await input({
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
        } else {
          contractName = await input({
            message: "Contract name (used for ABI file and YAML):",
            default: "TokenContract",
            validate: (v) =>
              /^[A-Za-z][A-Za-z0-9]*$/.test(v.trim()) ||
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
        } else {
          contractAddress = await input({
            message: "Contract address:",
            default: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
            validate: (v) =>
              /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x hex address",
          });
        }
      }
      contractAddress = contractAddress.trim();

      // Start Block
      let startBlockStr = opts.startBlock;
      if (!startBlockStr) {
        if (isNonInteractive) {
          startBlockStr = "0";
        } else {
          startBlockStr = await input({
            message: "Start block (indexing starting point):",
            default: "0",
            validate: (v) => /^\d+$/.test(v.trim()) || "Must be a non-negative integer",
          });
        }
      }
      const startBlock = parseInt(startBlockStr.trim(), 10);

      // Advanced engine and infrastructure settings
      let fallbackRpcUrls: string[] = [];
      if (opts.fallbackRpc) {
        fallbackRpcUrls = Array.isArray(opts.fallbackRpc)
          ? opts.fallbackRpc
          : opts.fallbackRpc.split(",").map((s: string) => s.trim()).filter(Boolean);
      }

      let webhookUrl: string | undefined = opts.webhookUrl;
      let ringBufferDepth: number | undefined = opts.reorgDepth
        ? parseInt(opts.reorgDepth, 10)
        : undefined;
      let reconcilerIntervalSecs: number | undefined = opts.reconcilerInterval
        ? parseInt(opts.reconcilerInterval, 10)
        : undefined;

      if (!isNonInteractive) {
        const wantAdvanced = await select<boolean>({
          message: "Configure advanced engine settings (fallback RPCs, webhooks, reorg buffer)?",
          choices: [
            { name: "No - use recommended production defaults", value: false },
            { name: "Yes - customize engine settings", value: true },
          ],
          default: false,
        });

        if (wantAdvanced) {
          const fallbackStr = await input({
            message: "Fallback RPC URLs (optional, comma-separated):",
            default: "",
          });
          if (fallbackStr.trim()) {
            fallbackRpcUrls = fallbackStr
              .split(",")
              .map((s) => s.trim())
              .filter(Boolean);
          }

          const whStr = await input({
            message: "Webhook URL (optional, e.g. https://api.mybackend.com/webhooks):",
            default: "",
          });
          if (whStr.trim()) {
            webhookUrl = whStr.trim();
          }

          const depthStr = await input({
            message: "Reorg ring buffer depth:",
            default: "128",
            validate: (v) =>
              (/^\d+$/.test(v.trim()) && parseInt(v.trim(), 10) >= 10) ||
              "Must be an integer >= 10",
          });
          ringBufferDepth = parseInt(depthStr.trim(), 10);
        }
      }

      const answers: InitAnswers = {
        projectName,
        networkName: networkKey,
        chainId: network.chainId,
        rpcUrl,
        fallbackRpcUrls,
        contractName,
        contractAddress,
        startBlock,
        webhookUrl,
        ringBufferDepth,
        reconcilerIntervalSecs,
      };

      ui.banner("Project Scaffolding", `Creating new indexer in ./${projectName}`);

      // Create directories
      fs.mkdirSync(path.join(targetDir, "abis"), { recursive: true });
      fs.mkdirSync(path.join(targetDir, "src"), { recursive: true });
      fs.mkdirSync(path.join(targetDir, "deploy"), { recursive: true });

      // Write core indexer files
      fs.writeFileSync(
        path.join(targetDir, "logrix.yaml"),
        logrixYamlTemplate(answers)
      );
      fs.writeFileSync(
        path.join(targetDir, "schema.graphql"),
        schemaGraphqlTemplate()
      );
      fs.writeFileSync(
        path.join(targetDir, "abis", `${contractName}.json`),
        JSON.stringify(ERC20_ABI, null, 2)
      );
      fs.writeFileSync(
        path.join(targetDir, "src", "mapping.ts"),
        mappingTemplate(contractName)
      );
      fs.writeFileSync(
        path.join(targetDir, "package.json"),
        JSON.stringify(packageJsonTemplate(projectName), null, 2)
      );
      fs.writeFileSync(
        path.join(targetDir, "tsconfig.json"),
        JSON.stringify(tsconfigTemplate(), null, 2)
      );
      fs.writeFileSync(
        path.join(targetDir, ".gitignore"),
        gitignoreTemplate()
      );
      fs.writeFileSync(
        path.join(targetDir, "README.md"),
        readmeTemplate(projectName)
      );

      // Write deployment configuration profiles
      fs.writeFileSync(
        path.join(targetDir, "deploy", "values-aws.yaml"),
        valuesAwsTemplate(answers)
      );
      fs.writeFileSync(
        path.join(targetDir, "deploy", "secrets.example.yaml"),
        secretsExampleTemplate(answers)
      );

      ui.success("Project scaffolded", `Created indexer in ./${projectName}`);

      ui.section("Development Workflow");
      console.log(`  ${pc.cyan("cd")} ${projectName}`);
      console.log(`  ${pc.cyan("npm install")}`);
      console.log(`  ${pc.cyan("npm run codegen")}`);
      console.log(`  ${pc.cyan("npm run validate")}`);
      console.log(`  ${pc.cyan("npm run build")}`);
      console.log(`  ${pc.cyan("npm run export-values")}`);

      ui.section("Deployment (Helm)");
      ui.label(
        "Self-Hosted:",
        `helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix -f indexer-values.yaml`
      );
      ui.label(
        "AWS (EKS):",
        `helm install ${projectName} oci://ghcr.io/vishal-770/charts/logrix -f deploy/values-aws.yaml -f indexer-values.yaml --set config.existingSecret=logrix-secrets`
      );
      console.log();
    });

  return cmd;
}
