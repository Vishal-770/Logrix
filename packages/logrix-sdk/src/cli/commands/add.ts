import { Command } from "commander";
import { input } from "@inquirer/prompts";
import * as fs from "fs";
import * as path from "path";
import pc from "picocolors";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { ERC20_ABI } from "../templates";
import { ui } from "../ui";
import { CliError } from "../errors";

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
        ui.info("Fetched verified ABI from block explorer", address);
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
        ui.info("Fetched verified ABI from Sourcify", address);
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
          events?: string;
          secret?: string;
          maxRetries?: string;
        }
      ) => {
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");

        if (!fs.existsSync(configPath)) {
          throw new CliError(
            "logrix.yaml not found",
            "Project manifest is missing in current directory",
            "Run this command from your indexer project root directory"
          );
        }

        if (type !== "contract" && type !== "webhook") {
          throw new CliError(
            `Unknown component type '${type}'`,
            "Supported component types: 'contract', 'webhook'",
            "Example: logrix add contract Token --address 0x... or logrix add webhook https://..."
          );
        }

        let config: any;
        try {
          config = parseYaml(fs.readFileSync(configPath, "utf-8"));
        } catch (e: any) {
          throw new CliError("Failed to parse logrix.yaml", e.message);
        }

        if (type === "webhook") {
          ui.banner("Add Webhook", "Registering event destination");

          let url = nameArg;
          if (!url) {
            if (process.stdin.isTTY) {
              url = await input({
                message: "Webhook Destination URL:",
                validate: (v) =>
                  /^https?:\/\/.+/.test(v.trim()) || "Must be a valid http:// or https:// URL",
              });
            } else {
              ui.error("Webhook URL required", "Usage: logrix add webhook <url>");
              process.exit(1);
            }
          }
          url = url.trim();
          if (!/^https?:\/\/.+/.test(url)) {
            ui.error("Invalid webhook URL", "URL must start with http:// or https://");
            process.exit(1);
          }

          const events = opts.events
            ? opts.events.split(",").map((s) => s.trim()).filter(Boolean)
            : ["reorg"];
          const maxRetries = parseInt(opts.maxRetries || "3", 10);

          if (!config.webhooks) {
            config.webhooks = [];
          }

          const webhookEntry: any = {
            url,
            events,
            maxRetries,
          };
          if (opts.secret) {
            webhookEntry.secret = opts.secret.trim();
          }

          config.webhooks.push(webhookEntry);
          fs.writeFileSync(configPath, stringifyYaml(config, { indent: 2 }));

          ui.success("Webhook registered in logrix.yaml", url);
          ui.label("Events", events.join(", "));
          ui.label("Max Retries", String(maxRetries));
          if (opts.secret) {
            ui.label("Signing Secret", "*** (configured)");
          }
          console.log();
          return;
        }

        ui.banner("Add Contract", "Registering contract data source");

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
          ui.error(
            `Contract '${contractName}' already exists`,
            "Duplicate contract names are not permitted in logrix.yaml."
          );
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
            ui.error(`ABI file not found: ${opts.abi}`);
            process.exit(1);
          }
          fs.copyFileSync(sourceAbi, targetAbiFile);
          ui.success("Imported ABI file", `abis/${contractName}.json`);
        } else if (opts.fetchAbi) {
          ui.info(`Fetching verified ABI for ${contractAddress} (Chain ID: ${chainId}) ...`);
          const abi = await fetchAbi(contractAddress, chainId, opts.etherscanKey);
          if (abi) {
            fs.writeFileSync(targetAbiFile, JSON.stringify(abi, null, 2));
            ui.success("Verified ABI downloaded", `abis/${contractName}.json`);
          } else {
            ui.warn(
              `Could not fetch ABI for ${contractAddress}`,
              "Using standard ERC-20 placeholder."
            );
            fs.writeFileSync(targetAbiFile, JSON.stringify(ERC20_ABI, null, 2));
          }
        } else if (!fs.existsSync(targetAbiFile)) {
          fs.writeFileSync(targetAbiFile, JSON.stringify(ERC20_ABI, null, 2));
          ui.info("Created ERC-20 placeholder ABI", `abis/${contractName}.json`);
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

        ui.success(`Contract '${contractName}' added to logrix.yaml`);
        ui.label("Contract Name", contractName);
        ui.label("Address", contractAddress);
        ui.label("Start Block", String(startBlock));
        ui.label("Network", `${networkName} (${chainId})`);

        ui.section("Next Steps");
        console.log(`  1. Review ABI at ${pc.cyan(`abis/${contractName}.json`)}`);
        console.log(`  2. Generate type bindings: ${pc.bold("logrix codegen")}`);
        console.log(`  3. Compile AssemblyScript handler: ${pc.bold("logrix build")}`);
        console.log();
      }
    );

  return cmd;
}
