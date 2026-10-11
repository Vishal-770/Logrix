"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.ui = void 0;
const picocolors_1 = __importDefault(require("picocolors"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
exports.ui = {
    banner(title, subtitle) {
        console.log();
        const brand = picocolors_1.default.bold(picocolors_1.default.cyan("Logrix"));
        const version = picocolors_1.default.dim("v0.4.0");
        const titleText = picocolors_1.default.bold(title);
        console.log(`  ${brand} ${version} ${picocolors_1.default.dim("|")} ${titleText}`);
        if (subtitle) {
            console.log(`  ${picocolors_1.default.dim(subtitle)}`);
        }
        console.log();
    },
    divider() {
        console.log(picocolors_1.default.dim("  " + "─".repeat(60)));
    },
    section(name) {
        console.log();
        console.log(`  ${picocolors_1.default.bold(picocolors_1.default.white(name))}`);
        console.log(picocolors_1.default.dim("  " + "─".repeat(60)));
    },
    success(message, detail) {
        const symbol = picocolors_1.default.green("✓");
        const msg = picocolors_1.default.bold(message);
        const det = detail ? picocolors_1.default.dim(` (${detail})`) : "";
        console.log(`  ${symbol} ${msg}${det}`);
    },
    error(message, detail, hint) {
        const symbol = picocolors_1.default.red("✕");
        const msg = picocolors_1.default.bold(picocolors_1.default.red(message));
        console.log(`  ${symbol} ${msg}`);
        if (detail) {
            console.log(`    ${picocolors_1.default.dim(detail)}`);
        }
        if (hint) {
            console.log(`    ${picocolors_1.default.yellow("Hint:")} ${picocolors_1.default.dim(hint)}`);
        }
    },
    warn(message, detail) {
        const symbol = picocolors_1.default.yellow("!");
        const msg = picocolors_1.default.yellow(message);
        const det = detail ? picocolors_1.default.dim(` (${detail})`) : "";
        console.log(`  ${symbol} ${msg}${det}`);
    },
    info(message, detail) {
        const symbol = picocolors_1.default.cyan("›");
        const msg = message;
        const det = detail ? picocolors_1.default.dim(` (${detail})`) : "";
        console.log(`  ${symbol} ${msg}${det}`);
    },
    step(current, total, message) {
        const progress = picocolors_1.default.dim(`[${current}/${total}]`);
        console.log(`  ${progress} ${picocolors_1.default.bold(message)}`);
    },
    spinner(text) {
        return (0, ora_1.default)({
            text,
            prefixText: " ",
            color: "cyan",
        }).start();
    },
    table(head, colWidths) {
        const tableOptions = {
            head: head.map((h) => picocolors_1.default.bold(picocolors_1.default.white(h))),
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
        return new cli_table3_1.default(tableOptions);
    },
    label(key, value, pad = 20) {
        const paddedKey = picocolors_1.default.dim(key.padEnd(pad));
        console.log(`  ${paddedKey} ${value}`);
    },
    latency(ms) {
        if (ms < 150) {
            return picocolors_1.default.green(`${ms} ms`);
        }
        else if (ms < 400) {
            return picocolors_1.default.yellow(`${ms} ms`);
        }
        else {
            return picocolors_1.default.red(`${ms} ms`);
        }
    },
    statusBadge(status) {
        switch (status) {
            case "ONLINE":
            case "DEPLOYED":
            case "PASSED":
                return picocolors_1.default.green(picocolors_1.default.bold(`✓ ${status}`));
            case "OFFLINE":
            case "EMPTY":
            case "FAILED":
                return picocolors_1.default.red(picocolors_1.default.bold(`✕ ${status}`));
            case "SKIPPED":
                return picocolors_1.default.dim(`- ${status}`);
        }
    },
};
