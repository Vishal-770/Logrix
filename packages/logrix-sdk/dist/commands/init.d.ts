import { Command } from "commander";
export interface NetworkOption {
    name: string;
    chainId: number;
    rpcUrl: string;
    category: "mainnet" | "testnet";
}
export declare const NETWORKS: Record<string, NetworkOption>;
export declare function initCommand(): Command;
