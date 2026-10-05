import { Command } from "commander";
import { input } from "@inquirer/prompts";
import * as fs from "fs";
import * as path from "path";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { ERC20_ABI } from "../templates";

const EXPLORER_APIS: Record<number, string> = {
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
async function fetchAbi(address: string, chainId: number = 1, apiKey?: string): Promise<any[] | null> {
  const addr = address.toLowerCase();

  // Try Etherscan-compatible explorer API
  const baseExplorer = EXPLORER_APIS[chainId] || EXPLORER_APIS[1];
  try {
    const keyParam = apiKey ? `&apikey=${apiKey}` : "";
    const url = `${baseExplorer}?module=contract&action=getabi&address=${addr}${keyParam}`;
    const resp = await fetch(url);
    const json: any = await resp.json();
    if (json.status === "1" && json.result) {
      const parsed = JSON.parse(json.result);
      if (Array.isArray(parsed) && parsed.length > 0) {
        console.log(`  Fetched verified ABI from block explorer for ${address}`);
        return parsed;
      }
    }
  } catch {
    // fall through to Sourcify
  }

  // Try Sourcify (supports any chainId, no API key required)
  try {
    const url = `https://repo.sourcify.dev/contracts/full_match/${chainId}/${addr}/metadata.json`;
    const resp = await fetch(url);
    if (resp.ok) {
      const json: any = await resp.json();
      const abi = json?.output?.abi;
      if (Array.isArray(abi) && abi.length > 0) {
        console.log(`  Fetched verified ABI from Sourcify for ${address}`);
        return abi;
      }
    }
  } catch {
    // fall through
  }

  return null;
}

export function addCommand(): Command {
  const cmd = new Command("add");

  cmd
    .description("Add components to an existing Logrix indexer project")
    .argument("<type>", "Type of component to add (e.g. contract)")
    .argument("[name]", "Name of the component")
    .option("--address <address>", "Contract address")
    .option("--abi <path>", "Path to ABI JSON file")
    .option("--fetch-abi", "Auto-fetch ABI from block explorer / Sourcify")
    .option("--etherscan-key <key>", "Explorer API key for ABI fetch (optional)")
    .option("--start-block <block>", "Starting block number", "0")
    .action(
      async (
        type: string,
        nameArg: string | undefined,
        opts: {
          address?: string;
          abi?: string;
          fetchAbi?: boolean;
          etherscanKey?: string;
          startBlock: string;
        }
      ) => {
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

        let config: any;
        try {
          config = parseYaml(fs.readFileSync(configPath, "utf-8"));
        } catch (e: any) {
          console.error(`Error parsing logrix.yaml: ${e.message}`);
          process.exit(1);
        }

        if (!config.dataSources) {
          config.dataSources = [];
        }

        const chainId = config.network?.chainId ?? 1;

        // 1. Resolve contract name
        let contractName = nameArg;
        if (!contractName) {
          if (process.stdin.isTTY) {
            contractName = await input({
              message: "Smart Contract Name:",
              default: "SecondaryContract",
              validate: (v) =>
                /^[A-Za-z][A-Za-z0-9]*$/.test(v.trim()) ||
                "Must be alphanumeric, starting with a letter",
            });
          } else {
            contractName = "SecondaryContract";
          }
        }
        contractName = contractName.trim();

        // Check for collision
        const exists = config.dataSources.some((ds: any) => ds.name === contractName);
        if (exists) {
          console.error(`Error: A contract named '${contractName}' is already defined in logrix.yaml.`);
          process.exit(1);
        }

        // 2. Resolve contract address
        let contractAddress = opts.address;
        if (!contractAddress) {
          if (process.stdin.isTTY) {
            contractAddress = await input({
              message: "Target Contract Address:",
              default: "0x0000000000000000000000000000000000000000",
              validate: (v) =>
                /^0x[0-9a-fA-F]{40}$/.test(v.trim()) || "Must be a valid 0x hex address",
            });
          } else {
            contractAddress = "0x0000000000000000000000000000000000000000";
          }
        }
        contractAddress = contractAddress.trim();

        // 3. Resolve start block
        let startBlockStr = opts.startBlock;
        if (!opts.address && process.stdin.isTTY) {
          startBlockStr = await input({
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
            console.error(`Error: Specified ABI file not found: ${opts.abi}`);
            process.exit(1);
          }
          fs.copyFileSync(sourceAbi, targetAbiFile);
          console.log(`  Copied ABI from ${opts.abi} to abis/${contractName}.json`);
        } else if (opts.fetchAbi) {
          console.log(`  Fetching ABI for ${contractAddress} (Chain ID: ${chainId}) ...`);
          const abi = await fetchAbi(contractAddress, chainId, opts.etherscanKey);
          if (abi) {
            fs.writeFileSync(targetAbiFile, JSON.stringify(abi, null, 2));
            console.log(`  Saved fetched ABI to abis/${contractName}.json`);
          } else {
            console.warn(
              `  Warning: Could not fetch ABI for ${contractAddress}. Using ERC-20 placeholder.`
            );
            fs.writeFileSync(targetAbiFile, JSON.stringify(ERC20_ABI, null, 2));
          }
        } else if (!fs.existsSync(targetAbiFile)) {
          fs.writeFileSync(targetAbiFile, JSON.stringify(ERC20_ABI, null, 2));
          console.log(`  Created default ERC-20 placeholder ABI at abis/${contractName}.json`);
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
        fs.writeFileSync(configPath, stringifyYaml(config, { indent: 2 }));

        console.log(`\nSuccessfully added contract '${contractName}' to logrix.yaml!`);
        console.log("Next steps:");
        console.log(`  1. Review / replace ABI at: abis/${contractName}.json`);
        console.log("  2. Run: logrix codegen");
        console.log("  3. Run: logrix build\n");
      }
    );

  return cmd;
}
