import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import { parse as parseYaml } from "yaml";

async function jsonRpcCall(url: string, method: string, params: any[] = []): Promise<{ result?: any; error?: any; latencyMs: number }> {
  const start = Date.now();
  try {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    });
    const latencyMs = Date.now() - start;
    if (!res.ok) {
      return { error: `HTTP ${res.status}: ${res.statusText}`, latencyMs };
    }
    const json: any = await res.json();
    return { result: json.result, error: json.error, latencyMs };
  } catch (e: any) {
    return { error: e.message, latencyMs: Date.now() - start };
  }
}

export function statusCommand(): Command {
  const cmd = new Command("status");

  cmd
    .description("Check RPC connectivity, network block height, and contract deployment status")
    .action(async () => {
      const projectDir = process.cwd();
      const configPath = path.join(projectDir, "logrix.yaml");

      if (!fs.existsSync(configPath)) {
        console.error("Error: logrix.yaml not found. Run this command from your project root.");
        process.exit(1);
      }

      let config: any;
      try {
        config = parseYaml(fs.readFileSync(configPath, "utf-8"));
      } catch (e: any) {
        console.error(`Error parsing logrix.yaml: ${e.message}`);
        process.exit(1);
      }

      const networkName = config.network?.name || "unspecified";
      const configuredChainId = config.network?.chainId;
      const rpcUrl = config.network?.rpcUrl;

      console.log("\nLogrix Indexer Status");
      console.log("============================================================");
      console.log(`Network Name:     ${networkName}`);
      console.log(`Configured Chain: ${configuredChainId ?? "Not set"}`);
      console.log(`RPC Endpoint:     ${rpcUrl || "None"}`);

      if (!rpcUrl) {
        console.error("\nError: No RPC URL configured in logrix.yaml.");
        console.log("============================================================\n");
        process.exit(1);
      }

      // 1. Query block height
      process.stdout.write("Connecting to RPC endpoint ... ");
      const blockRes = await jsonRpcCall(rpcUrl, "eth_blockNumber");

      if (blockRes.error || !blockRes.result) {
        console.log("FAILED");
        console.error(`RPC Error: ${JSON.stringify(blockRes.error)} (${blockRes.latencyMs} ms)`);
        console.log("============================================================\n");
        process.exit(1);
      }

      const latestBlock = parseInt(blockRes.result, 16);
      console.log(`ONLINE (${blockRes.latencyMs} ms)`);
      console.log(`Latest Block:     #${latestBlock.toLocaleString()}`);

      // 2. Query chain ID
      const chainRes = await jsonRpcCall(rpcUrl, "eth_chainId");
      if (chainRes.result) {
        const reportedChainId = parseInt(chainRes.result, 16);
        const match = configuredChainId === reportedChainId;
        console.log(
          `Chain ID Check:   Reported ${reportedChainId} (${match ? "VERIFIED MATCH" : "MISMATCH WARNING"})`
        );
      }

      // 3. Inspect contracts
      const dataSources = config.dataSources || [];
      console.log(`\nContracts Monitored (${dataSources.length}):`);
      console.log("------------------------------------------------------------");

      for (const ds of dataSources) {
        const addr = ds.source?.address || "";
        const name = ds.name || "Unnamed";
        const startBlock = ds.source?.startBlock ?? 0;

        if (!addr) {
          console.log(`  - ${name}: [NO ADDRESS SPECIFIED]`);
          continue;
        }

        const codeRes = await jsonRpcCall(rpcUrl, "eth_getCode", [addr, "latest"]);
        const code = codeRes.result || "";
        const isDeployed = code.length > 2 && code !== "0x" && code !== "0x0";
        const byteSize = isDeployed ? (code.length - 2) / 2 : 0;

        console.log(`  - Name:        ${name}`);
        console.log(`    Address:     ${addr}`);
        console.log(
          `    Deployed:    ${isDeployed ? `YES (${byteSize.toLocaleString()} bytes bytecode)` : "NO (Empty bytecode / Not found)"}`
        );
        console.log(`    Start Block: #${startBlock.toLocaleString()}`);
        if (latestBlock > startBlock) {
          const lag = latestBlock - startBlock;
          console.log(`    Sync Scope:  #${startBlock} -> #${latestBlock} (${lag.toLocaleString()} blocks to process)`);
        }
        console.log("");
      }

      console.log("============================================================\n");
    });

  return cmd;
}
