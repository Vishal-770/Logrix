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
    .action(async (nameArg: string | undefined) => {
      console.log("\nLogrix Project Initializer\n");

      const projectName = await input({
        message: "Project name:",
        default: nameArg ?? "my-indexer",
        validate: (v) => v.trim().length > 0 || "Name cannot be empty",
      });

      const targetDir = path.resolve(process.cwd(), projectName);
      if (fs.existsSync(targetDir)) {
        console.error(`Error: directory '${projectName}' already exists.`);
        process.exit(1);
      }

      const networkKey = await select<string>({
        message: "Network:",
        choices: Object.entries(NETWORKS).map(([key, net]) => ({
          name: `${net.name} (chain ${net.chainId})`,
          value: key,
        })),
        default: "arbitrum-one",
      });

      const network = NETWORKS[networkKey];

      const rpcUrl = await input({
        message: "RPC URL:",
        default: network.rpcUrl,
        validate: (v) => v.trim().length > 0 || "RPC URL cannot be empty",
      });

      const contractName = await input({
        message: "Contract name (used for ABI file and YAML):",
        default: "TokenContract",
        validate: (v) =>
          /^[A-Za-z][A-Za-z0-9]*$/.test(v.trim()) ||
          "Must be alphanumeric, starting with a letter",
      });

      const contractAddress = await input({
        message: "Contract address:",
        default: "0x0000000000000000000000000000000000000000",
        validate: (v) =>
          /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x address",
      });

      const startBlockStr = await input({
        message: "Start block:",
        default: "0",
        validate: (v) => /^\d+$/.test(v.trim()) || "Must be a non-negative integer",
      });

      const answers: InitAnswers = {
        projectName,
        networkName: network.name,
        chainId: network.chainId,
        rpcUrl: rpcUrl.trim(),
        contractName: contractName.trim(),
        contractAddress: contractAddress.trim(),
        startBlock: parseInt(startBlockStr.trim(), 10),
      };

      console.log(`\nScaffolding in ./${projectName}/ ...\n`);

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
      console.log("  npm run build\n");
    });

  return cmd;
}
