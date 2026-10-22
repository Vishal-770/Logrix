export declare class CliError extends Error {
    readonly detail?: string;
    readonly hint?: string;
    readonly exitCode: number;
    constructor(message: string, detail?: string, hint?: string, exitCode?: number);
}
export declare function handleCliError(err: unknown): void;
