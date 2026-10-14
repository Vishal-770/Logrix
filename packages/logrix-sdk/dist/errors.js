"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.CliError = void 0;
exports.handleCliError = handleCliError;
const picocolors_1 = __importDefault(require("picocolors"));
class CliError extends Error {
    constructor(message, detail, hint, exitCode = 1) {
        super(message);
        this.name = "CliError";
        this.detail = detail;
        this.hint = hint;
        this.exitCode = exitCode;
        Object.setPrototypeOf(this, CliError.prototype);
    }
}
exports.CliError = CliError;
function handleCliError(err) {
    if (err instanceof CliError) {
        console.error();
        console.error(`  ${picocolors_1.default.red("[x]")} ${picocolors_1.default.bold(picocolors_1.default.red(err.message))}`);
        if (err.detail) {
            console.error(`    ${picocolors_1.default.dim(err.detail)}`);
        }
        if (err.hint) {
            console.error(`    ${picocolors_1.default.yellow("Hint:")} ${picocolors_1.default.dim(err.hint)}`);
        }
        console.error();
        process.exit(err.exitCode);
    }
    else if (err instanceof Error) {
        console.error();
        console.error(`  ${picocolors_1.default.red("[x]")} ${picocolors_1.default.bold(picocolors_1.default.red(err.message))}`);
        if (process.env.DEBUG && err.stack) {
            console.error(picocolors_1.default.dim(err.stack));
        }
        console.error();
        process.exit(1);
    }
    else {
        console.error();
        console.error(`  ${picocolors_1.default.red("[x]")} ${picocolors_1.default.bold(picocolors_1.default.red(String(err)))}`);
        console.error();
        process.exit(1);
    }
}
