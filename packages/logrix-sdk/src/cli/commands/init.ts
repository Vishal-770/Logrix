import { Command } from "commander";
import { input, select } from "@inquirer/prompts";
import * as fs from "fs";
import * as path from "path";
import {
  InitAnswers,
  logrixYamlTemplate,
  schemaGraphqlTemplate,
  mappingTemplate,
  packageJsonTemplate,
  tsconfigTemplate,
  gitignoreTemplate,
  readmeTemplate,
  ERC20_ABI,
} from "../templates";

interface NetworkOption {
  name: string;
  chainId: number;
  rpcUrl: string;
}

const NETWORKS: Record<string, NetworkOption> = {
  "ethereum": { name: "ethereum", chainId: 1, rpcUrl: "https://eth.llamarpc.com" },
  "arbitrum-one": { name: "arbitrum-one", chainId: 42161, rpcUrl: "https://arb1.arbitrum.io/rpc" },
  "base": { name: "base", chainId: 8453, rpcUrl: "https://mainnet.base.org" },
  "polygon": { name: "polygon", chainId: 137, rpcUrl: "https://polygon-rpc.com" },
  "arbitrum-sepolia": { name: "arbitrum-sepolia", chainId: 421614, rpcUrl: "https://sepolia-rollup.arbitrum.io/rpc" },
  "sepolia": { name: "sepolia", chainId: 11155111, rpcUrl: "https://rpc.sepolia.org" },
};

export function initCommand(): Command {
  const cmd = new Command("init");

  cmd
    .description("Scaffold a new Logrix indexer project")
    .argument("[name]", "Project directory name")
    .option("-y, --yes", "Skip prompts and use defaults (non-interactive)")
    .option("--network <network>", "Network preset name (e.g. arbitrum-one, ethereum, base)")
    .option("--rpc <url>", "Custom RPC endpoint URL")
    .option("--contract-name <name>", "Contract name")
    .option("--address <address>", "Contract target address")
    .option("--start-block <block>", "Starting block number")
    .action(async (nameArg: string | undefined, opts: any) => {
      console.log("\nLogrix Project Initializer\n");

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
        console.error(`Error: directory '${projectName}' already exists.`);
        process.exit(1);
      }

      // Network selection
      let networkKey = opts.network;
      if (!networkKey || !NETWORKS[networkKey]) {
        if (isNonInteractive) {
          networkKey = "arbitrum-one";
        } else {
          networkKey = await select<string>({
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
              /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x address",
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
            message: "Start block:",
            default: "0",
            validate: (v) => /^\d+$/.test(v.trim()) || "Must be a non-negative integer",
          });
        }
      }

      const answers: InitAnswers = {
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
      fs.writeFileSync(
        path.join(targetDir, "logrix.yaml"),
        logrixYamlTemplate(answers)
      );

      // schema.graphql
      fs.writeFileSync(
        path.join(targetDir, "schema.graphql"),
        schemaGraphqlTemplate()
      );

      // Placeholder ABI
      fs.writeFileSync(
        path.join(targetDir, "abis", `${answers.contractName}.json`),
        JSON.stringify(ERC20_ABI, null, 2)
      );

      // src/mapping.ts
      fs.writeFileSync(
        path.join(targetDir, "src", "mapping.ts"),
        mappingTemplate(answers.contractName)
      );

      // package.json
      fs.writeFileSync(
        path.join(targetDir, "package.json"),
        JSON.stringify(packageJsonTemplate(projectName), null, 2)
      );

      // tsconfig.json (AS config)
      fs.writeFileSync(
        path.join(targetDir, "tsconfig.json"),
        JSON.stringify(tsconfigTemplate(), null, 2)
      );

      // .gitignore
      fs.writeFileSync(path.join(targetDir, ".gitignore"), gitignoreTemplate());

      // README.md
      fs.writeFileSync(path.join(targetDir, "README.md"), readmeTemplate(projectName));

      console.log("Done. Next steps:\n");
      console.log(`  cd ${projectName}`);
      console.log("  npm install");
      console.log("  npm run codegen");
      console.log("  npm run validate");
      console.log("  npm run build\n");
    });

  return cmd;
}
