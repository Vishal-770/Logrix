import { Command } from "commander";
import * as fs from "fs";
import * as path from "path";
import pc from "picocolors";
import { parse as parseYaml } from "yaml";
import { ui } from "../ui";
import { CliError } from "../errors";

function maskUrl(rawUrl: string): string {
  try {
    const parsed = new URL(rawUrl);
    if (parsed.password) parsed.password = "******";
    for (const key of Array.from(parsed.searchParams.keys())) {
      if (/key|token|secret|auth/i.test(key)) {
        parsed.searchParams.set(key, "******");
      }
    }
    const parts = parsed.pathname.split("/");
    if (parts.length > 2 && (parts[1] === "v2" || parts[1] === "v3")) {
      parts[2] = "******";
      parsed.pathname = parts.join("/");
    }
    return parsed.toString();
  } catch {
    return rawUrl;
  }
}

async function jsonRpcCall(
  url: string,
  method: string,
  params: any[] = [],
  timeoutMs: number = 10000
): Promise<{ result?: any; error?: any; latencyMs: number }> {
  const start = Date.now();
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
      signal: controller.signal,
    });
    clearTimeout(timer);
    const latencyMs = Date.now() - start;
    if (!res.ok) {
      return { error: `HTTP ${res.status}: ${res.statusText}`, latencyMs };
    }
    const json: any = await res.json();
    return { result: json.result, error: json.error, latencyMs };
  } catch (e: any) {
    const msg = e.name === "AbortError" ? "Request timed out after 10s" : e.message;
    return { error: msg, latencyMs: Date.now() - start };
  }
}

export function statusCommand(): Command {
  const cmd = new Command("status");

  cmd
    .description(
      "Check RPC connectivity, network block height, and contract deployment status"
    )
    .action(async () => {
      const projectDir = process.cwd();
      const configPath = path.join(projectDir, "logrix.yaml");

      ui.banner("Indexer Diagnostics", "Inspecting live blockchain endpoints and contracts");

      if (!fs.existsSync(configPath)) {
        throw new CliError(
          "logrix.yaml not found",
          "Project manifest is missing in current directory",
          "Command must be executed from your indexer project root"
        );
      }

      let config: any;
      try {
        config = parseYaml(fs.readFileSync(configPath, "utf-8"));
      } catch (e: any) {
        throw new CliError("Failed to parse logrix.yaml", e.message);
      }

      const networkName = config.network?.name || "unspecified";
      const configuredChainId = config.network?.chainId;
      const rpcUrl = config.network?.rpcUrl;

      ui.section("Network & RPC Configuration");
      ui.label("Network:", pc.bold(pc.white(networkName)));
      ui.label("Configured Chain:", pc.cyan(String(configuredChainId ?? "Not set")));
      ui.label("Primary RPC:", rpcUrl ? pc.dim(maskUrl(rpcUrl)) : pc.red("None configured"));

      if (!rpcUrl) {
        throw new CliError(
          "Missing RPC URL",
          "network.rpcUrl is not defined in logrix.yaml",
          "Define network.rpcUrl in logrix.yaml to run status checks"
        );
      }

      const spinner = ui.spinner("Probing blockchain RPC endpoints...");

      // 1. Query block height
      const blockRes = await jsonRpcCall(rpcUrl, "eth_blockNumber");
      if (blockRes.error || !blockRes.result) {
        spinner.fail("Primary RPC Unreachable");
        throw new CliError(
          "Primary RPC Unreachable",
          typeof blockRes.error === "object" ? JSON.stringify(blockRes.error) : String(blockRes.error),
          "Verify RPC endpoint availability, credentials, or rate limits"
        );
      }

      const latestBlock = parseInt(blockRes.result, 16);

      // 2. Query chain ID
      const chainRes = await jsonRpcCall(rpcUrl, "eth_chainId");
      spinner.succeed("Blockchain endpoints reachable");

      ui.label(
        "Primary Status:",
        `${ui.statusBadge("ONLINE")} (${ui.latency(blockRes.latencyMs)})`
      );
      ui.label("Latest Block:", pc.bold(pc.green(`#${latestBlock.toLocaleString()}`)));

      if (chainRes.result) {
        const reportedChainId = parseInt(chainRes.result, 16);
        const match = configuredChainId === reportedChainId;
        if (!match) {
          throw new CliError(
            `Chain ID Mismatch: Configured chain ID ${configuredChainId}, but node reported ${reportedChainId}`,
            "The configured network in logrix.yaml does not match the connected node.",
            "Update network.chainId or network.rpcUrl in logrix.yaml"
          );
        }
        ui.label("Chain ID Match:", pc.green(`[OK] Matched (#${reportedChainId})`));
      }

      // Check Fallback RPCs
      const fallbackUrls = config.network?.fallbackRpcUrls || [];
      if (fallbackUrls.length > 0) {
        ui.section(`Fallback RPC Gateway (${fallbackUrls.length} configured)`);
        const fbTable = ui.table(["Endpoint", "Status", "Latency", "Block Height"]);
        for (const fbUrl of fallbackUrls) {
          const masked = maskUrl(fbUrl);
          const fbRes = await jsonRpcCall(fbUrl, "eth_blockNumber");
          if (fbRes.error || !fbRes.result) {
            fbTable.push([
              pc.dim(masked.length > 35 ? masked.slice(0, 32) + "..." : masked),
              ui.statusBadge("OFFLINE"),
              pc.dim("-"),
              pc.dim(String(fbRes.error || "unreachable")),
            ]);
          } else {
            const fbBlock = parseInt(fbRes.result, 16);
            fbTable.push([
              pc.cyan(masked.length > 35 ? masked.slice(0, 32) + "..." : masked),
              ui.statusBadge("ONLINE"),
              ui.latency(fbRes.latencyMs),
              pc.bold(pc.green(`#${fbBlock.toLocaleString()}`)),
            ]);
          }
        }
        console.log(fbTable.toString());
      }

      // 3. Inspect contracts
      const dataSources = config.dataSources || [];
      ui.section(`Monitored Contracts (${dataSources.length})`);

      const contractTable = ui.table([
        "Contract",
        "Address",
        "Status",
        "Start Block",
        "Sync Delta",
      ]);

      for (const ds of dataSources) {
        const addr = ds.source?.address || "";
        const name = ds.name || "Unnamed";
        const startBlock = ds.source?.startBlock ?? 0;

        if (!addr) {
          contractTable.push([
            name,
            pc.yellow("Missing address"),
            ui.statusBadge("EMPTY"),
            pc.dim(String(startBlock)),
            pc.dim("-"),
          ]);
          continue;
        }

        const codeRes = await jsonRpcCall(rpcUrl, "eth_getCode", [addr, "latest"]);
        const code = codeRes.result || "";
        const isDeployed = code.length > 2 && code !== "0x" && code !== "0x0";
        const byteSize = isDeployed ? (code.length - 2) / 2 : 0;

        const statusStr = isDeployed
          ? `${ui.statusBadge("DEPLOYED")} ${pc.dim(`(${byteSize}B)`)}`
          : ui.statusBadge("EMPTY");

        const deltaStr =
          latestBlock > startBlock
            ? pc.green(`+${(latestBlock - startBlock).toLocaleString()} blks`)
            : pc.dim("0 blks");

        contractTable.push([
          pc.bold(name),
          pc.dim(addr.slice(0, 10) + "..." + addr.slice(-8)),
          statusStr,
          pc.cyan(`#${startBlock.toLocaleString()}`),
          deltaStr,
        ]);
      }
      console.log(contractTable.toString());

      // Webhooks Summary
      const webhooks = config.webhooks || [];
      if (webhooks.length > 0) {
        ui.section(`Declarative Webhooks (${webhooks.length})`);
        const whTable = ui.table(["Endpoint URL", "Subscribed Events", "Max Retries"]);
        for (const wh of webhooks) {
          whTable.push([
            pc.cyan(wh.url),
            (wh.events || ["reorg"]).join(", "),
            String(wh.maxRetries || 3),
          ]);
        }
        console.log(whTable.toString());
      }

      ui.divider();
      console.log();
    });

  return cmd;
}
