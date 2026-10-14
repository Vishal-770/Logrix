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
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.statusCommand = statusCommand;
const commander_1 = require("commander");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const picocolors_1 = __importDefault(require("picocolors"));
const yaml_1 = require("yaml");
const ui_1 = require("../ui");
const errors_1 = require("../errors");
async function jsonRpcCall(url, method, params = []) {
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
        const json = await res.json();
        return { result: json.result, error: json.error, latencyMs };
    }
    catch (e) {
        return { error: e.message, latencyMs: Date.now() - start };
    }
}
function statusCommand() {
    const cmd = new commander_1.Command("status");
    cmd
        .description("Check RPC connectivity, network block height, and contract deployment status")
        .action(async () => {
        const projectDir = process.cwd();
        const configPath = path.join(projectDir, "logrix.yaml");
        ui_1.ui.banner("Indexer Diagnostics", "Inspecting live blockchain endpoints and contracts");
        if (!fs.existsSync(configPath)) {
            throw new errors_1.CliError("logrix.yaml not found", "Project manifest is missing in current directory", "Command must be executed from your indexer project root");
        }
        let config;
        try {
            config = (0, yaml_1.parse)(fs.readFileSync(configPath, "utf-8"));
        }
        catch (e) {
            throw new errors_1.CliError("Failed to parse logrix.yaml", e.message);
        }
        const networkName = config.network?.name || "unspecified";
        const configuredChainId = config.network?.chainId;
        const rpcUrl = config.network?.rpcUrl;
        ui_1.ui.section("Network & RPC Configuration");
        ui_1.ui.label("Network:", picocolors_1.default.bold(picocolors_1.default.white(networkName)));
        ui_1.ui.label("Configured Chain:", picocolors_1.default.cyan(String(configuredChainId ?? "Not set")));
        ui_1.ui.label("Primary RPC:", rpcUrl ? picocolors_1.default.dim(rpcUrl) : picocolors_1.default.red("None configured"));
        if (!rpcUrl) {
            throw new errors_1.CliError("Missing RPC URL", "network.rpcUrl is not defined in logrix.yaml", "Define network.rpcUrl in logrix.yaml to run status checks");
        }
        const spinner = ui_1.ui.spinner("Probing blockchain RPC endpoints...");
        // 1. Query block height
        const blockRes = await jsonRpcCall(rpcUrl, "eth_blockNumber");
        if (blockRes.error || !blockRes.result) {
            spinner.fail("Primary RPC Unreachable");
            throw new errors_1.CliError("Primary RPC Unreachable", typeof blockRes.error === "object" ? JSON.stringify(blockRes.error) : String(blockRes.error), "Verify RPC endpoint availability, credentials, or rate limits");
        }
        const latestBlock = parseInt(blockRes.result, 16);
        // 2. Query chain ID
        const chainRes = await jsonRpcCall(rpcUrl, "eth_chainId");
        spinner.succeed("Blockchain endpoints reachable");
        ui_1.ui.label("Primary Status:", `${ui_1.ui.statusBadge("ONLINE")} (${ui_1.ui.latency(blockRes.latencyMs)})`);
        ui_1.ui.label("Latest Block:", picocolors_1.default.bold(picocolors_1.default.green(`#${latestBlock.toLocaleString()}`)));
        if (chainRes.result) {
            const reportedChainId = parseInt(chainRes.result, 16);
            const match = configuredChainId === reportedChainId;
            const chainBadge = match
                ? picocolors_1.default.green(`[OK] Matched (#${reportedChainId})`)
                : picocolors_1.default.yellow(`! Mismatch (Configured: ${configuredChainId}, Node: ${reportedChainId})`);
            ui_1.ui.label("Chain ID Match:", chainBadge);
        }
        // Check Fallback RPCs
        const fallbackUrls = config.network?.fallbackRpcUrls || [];
        if (fallbackUrls.length > 0) {
            ui_1.ui.section(`Fallback RPC Gateway (${fallbackUrls.length} configured)`);
            const fbTable = ui_1.ui.table(["Endpoint", "Status", "Latency", "Block Height"]);
            for (const fbUrl of fallbackUrls) {
                const fbRes = await jsonRpcCall(fbUrl, "eth_blockNumber");
                if (fbRes.error || !fbRes.result) {
                    fbTable.push([
                        picocolors_1.default.dim(fbUrl.length > 35 ? fbUrl.slice(0, 32) + "..." : fbUrl),
                        ui_1.ui.statusBadge("OFFLINE"),
                        picocolors_1.default.dim("-"),
                        picocolors_1.default.dim(String(fbRes.error || "unreachable")),
                    ]);
                }
                else {
                    const fbBlock = parseInt(fbRes.result, 16);
                    fbTable.push([
                        picocolors_1.default.cyan(fbUrl.length > 35 ? fbUrl.slice(0, 32) + "..." : fbUrl),
                        ui_1.ui.statusBadge("ONLINE"),
                        ui_1.ui.latency(fbRes.latencyMs),
                        picocolors_1.default.bold(picocolors_1.default.green(`#${fbBlock.toLocaleString()}`)),
                    ]);
                }
            }
            console.log(fbTable.toString());
        }
        // 3. Inspect contracts
        const dataSources = config.dataSources || [];
        ui_1.ui.section(`Monitored Contracts (${dataSources.length})`);
        const contractTable = ui_1.ui.table([
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
                    picocolors_1.default.yellow("Missing address"),
                    ui_1.ui.statusBadge("EMPTY"),
                    picocolors_1.default.dim(String(startBlock)),
                    picocolors_1.default.dim("-"),
                ]);
                continue;
            }
            const codeRes = await jsonRpcCall(rpcUrl, "eth_getCode", [addr, "latest"]);
            const code = codeRes.result || "";
            const isDeployed = code.length > 2 && code !== "0x" && code !== "0x0";
            const byteSize = isDeployed ? (code.length - 2) / 2 : 0;
            const statusStr = isDeployed
                ? `${ui_1.ui.statusBadge("DEPLOYED")} ${picocolors_1.default.dim(`(${byteSize}B)`)}`
                : ui_1.ui.statusBadge("EMPTY");
            const deltaStr = latestBlock > startBlock
                ? picocolors_1.default.green(`+${(latestBlock - startBlock).toLocaleString()} blks`)
                : picocolors_1.default.dim("0 blks");
            contractTable.push([
                picocolors_1.default.bold(name),
                picocolors_1.default.dim(addr.slice(0, 10) + "..." + addr.slice(-8)),
                statusStr,
                picocolors_1.default.cyan(`#${startBlock.toLocaleString()}`),
                deltaStr,
            ]);
        }
        console.log(contractTable.toString());
        // Webhooks Summary
        const webhooks = config.webhooks || [];
        if (webhooks.length > 0) {
            ui_1.ui.section(`Declarative Webhooks (${webhooks.length})`);
            const whTable = ui_1.ui.table(["Endpoint URL", "Subscribed Events", "Max Retries"]);
            for (const wh of webhooks) {
                whTable.push([
                    picocolors_1.default.cyan(wh.url),
                    (wh.events || ["reorg"]).join(", "),
                    String(wh.maxRetries || 3),
                ]);
            }
            console.log(whTable.toString());
        }
        ui_1.ui.divider();
        console.log();
    });
    return cmd;
}
