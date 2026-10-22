/**
 * Keep environment references symbolic while generating deployment artifacts.
 * The runtime pod resolves them after loading its environment.
 */
export declare function preserveEnvReference(val: string): string;
/** Return environment variable names referenced by strings in a value, in source order. */
export declare function extractEnvVarNames(value: unknown): string[];
export interface ConvertedRuntimeManifest {
    contracts: any[];
    webhooks: any[];
    minStartBlock: number;
    manifest: any;
    requiredEnvVars: string[];
}
/**
 * Converts high-level SDK project manifest to concrete Rust runtime engine format.
 */
export declare function convertSdkManifestToRuntime(config: any, chainId: number): ConvertedRuntimeManifest;
/**
 * Maps optional SDK indexing settings to Helm deployment values.
 */
export declare function mapIndexingToHelmValues(values: any, idx?: any): void;
/**
 * Resolves the primary AssemblyScript handler source entry point for an indexer project.
 * Checks mapping.source, fallback to mapping.file (if ending in .ts), or default "src/mapping.ts".
 */
export declare function resolveMappingSource(projectDir: string, config?: any): string;
