import { Command } from "commander";
/**
 * logrix export-values
 *
 * Reads logrix.yaml, schema.graphql, and build/mapping.wasm then writes an
 * indexer-values.yaml that is ready to be passed to:
 *   helm install logrix oci://ghcr.io/vishal-770/charts/logrix \
 *     -f indexer-values.yaml
 */
export declare function exportValuesCommand(): Command;
