import pc from "picocolors";
import ora, { Ora } from "ora";
import Table from "cli-table3";

export const ui = {
  banner(title: string, subtitle?: string): void {
    console.log();
    const brand = pc.bold(pc.cyan("Logrix"));
    const version = pc.dim("v0.4.0");
    const titleText = pc.bold(title);
    console.log(`  ${brand} ${version} ${pc.dim("|")} ${titleText}`);
    if (subtitle) {
      console.log(`  ${pc.dim(subtitle)}`);
    }
    console.log();
  },

  divider(): void {
    console.log(pc.dim("  " + "─".repeat(60)));
  },

  section(name: string): void {
    console.log();
    console.log(`  ${pc.bold(pc.white(name))}`);
    console.log(pc.dim("  " + "─".repeat(60)));
  },

  success(message: string, detail?: string): void {
    const symbol = pc.green("✓");
    const msg = pc.bold(message);
    const det = detail ? pc.dim(` (${detail})`) : "";
    console.log(`  ${symbol} ${msg}${det}`);
  },

  error(message: string, detail?: string, hint?: string): void {
    const symbol = pc.red("✕");
    const msg = pc.bold(pc.red(message));
    console.log(`  ${symbol} ${msg}`);
    if (detail) {
      console.log(`    ${pc.dim(detail)}`);
    }
    if (hint) {
      console.log(`    ${pc.yellow("Hint:")} ${pc.dim(hint)}`);
    }
  },

  warn(message: string, detail?: string): void {
    const symbol = pc.yellow("!");
    const msg = pc.yellow(message);
    const det = detail ? pc.dim(` (${detail})`) : "";
    console.log(`  ${symbol} ${msg}${det}`);
  },

  info(message: string, detail?: string): void {
    const symbol = pc.cyan("›");
    const msg = message;
    const det = detail ? pc.dim(` (${detail})`) : "";
    console.log(`  ${symbol} ${msg}${det}`);
  },

  step(current: number, total: number, message: string): void {
    const progress = pc.dim(`[${current}/${total}]`);
    console.log(`  ${progress} ${pc.bold(message)}`);
  },

  spinner(text: string): Ora {
    return ora({
      text,
      prefixText: " ",
      color: "cyan",
    }).start();
  },

  table(head: string[], colWidths?: number[]): Table.Table {
    const tableOptions: any = {
      head: head.map((h) => pc.bold(pc.white(h))),
      chars: {
        top: "─",
        "top-mid": "┬",
        "top-left": "┌",
        "top-right": "┐",
        bottom: "─",
        "bottom-mid": "┴",
        "bottom-left": "└",
        "bottom-right": "┘",
        left: "│",
        "left-mid": "├",
        mid: "─",
        "mid-mid": "┼",
        right: "│",
        "right-mid": "┤",
        middle: "│",
      },
      style: {
        head: [],
        border: ["gray"],
      },
    };
    if (colWidths) {
      tableOptions.colWidths = colWidths;
    }
    return new Table(tableOptions);
  },

  label(key: string, value: string, pad = 20): void {
    const paddedKey = pc.dim(key.padEnd(pad));
    console.log(`  ${paddedKey} ${value}`);
  },

  latency(ms: number): string {
    if (ms < 150) {
      return pc.green(`${ms} ms`);
    } else if (ms < 400) {
      return pc.yellow(`${ms} ms`);
    } else {
      return pc.red(`${ms} ms`);
    }
  },

  statusBadge(status: "ONLINE" | "OFFLINE" | "DEPLOYED" | "EMPTY" | "PASSED" | "FAILED" | "SKIPPED"): string {
    switch (status) {
      case "ONLINE":
      case "DEPLOYED":
      case "PASSED":
        return pc.green(pc.bold(`✓ ${status}`));
      case "OFFLINE":
      case "EMPTY":
      case "FAILED":
        return pc.red(pc.bold(`✕ ${status}`));
      case "SKIPPED":
        return pc.dim(`- ${status}`);
    }
  },
};
