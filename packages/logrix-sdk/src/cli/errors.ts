import pc from "picocolors";

export class CliError extends Error {
  public readonly detail?: string;
  public readonly hint?: string;
  public readonly exitCode: number;

  constructor(message: string, detail?: string, hint?: string, exitCode = 1) {
    super(message);
    this.name = "CliError";
    this.detail = detail;
    this.hint = hint;
    this.exitCode = exitCode;
    Object.setPrototypeOf(this, CliError.prototype);
  }
}

export function handleCliError(err: unknown): void {
  if (err instanceof CliError) {
    console.error();
    console.error(`  ${pc.red("✕")} ${pc.bold(pc.red(err.message))}`);
    if (err.detail) {
      console.error(`    ${pc.dim(err.detail)}`);
    }
    if (err.hint) {
      console.error(`    ${pc.yellow("Hint:")} ${pc.dim(err.hint)}`);
    }
    console.error();
    process.exit(err.exitCode);
  } else if (err instanceof Error) {
    console.error();
    console.error(`  ${pc.red("✕")} ${pc.bold(pc.red(err.message))}`);
    if (process.env.DEBUG && err.stack) {
      console.error(pc.dim(err.stack));
    }
    console.error();
    process.exit(1);
  } else {
    console.error();
    console.error(`  ${pc.red("✕")} ${pc.bold(pc.red(String(err)))}`);
    console.error();
    process.exit(1);
  }
}
