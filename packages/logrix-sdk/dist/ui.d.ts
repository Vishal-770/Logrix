import { Ora } from "ora";
import Table from "cli-table3";
export declare const ui: {
    banner(title: string, subtitle?: string): void;
    divider(): void;
    section(name: string): void;
    success(message: string, detail?: string): void;
    error(message: string, detail?: string, hint?: string): void;
    warn(message: string, detail?: string): void;
    info(message: string, detail?: string): void;
    step(current: number, total: number, message: string): void;
    spinner(text: string): Ora;
    table(head: string[], colWidths?: number[]): Table.Table;
    label(key: string, value: string, pad?: number): void;
    latency(ms: number): string;
    statusBadge(status: "ONLINE" | "OFFLINE" | "DEPLOYED" | "EMPTY" | "PASSED" | "FAILED" | "SKIPPED"): string;
};
