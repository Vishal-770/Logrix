export interface InitAnswers {
    projectName: string;
    networkName: string;
    chainId: number;
    rpcUrl: string;
    fallbackRpcUrls?: string[];
    contractName: string;
    contractAddress: string;
    startBlock: number;
    webhookUrl?: string;
    webhookEvents?: string[];
    secretEnvVars?: string[];
    ringBufferDepth?: number;
    reconcilerIntervalSecs?: number;
    fuelLimit?: number;
    maxMemoryMb?: number;
}
export declare function logrixYamlTemplate(a: InitAnswers): string;
export declare function schemaGraphqlTemplate(): string;
export declare function mappingTemplate(contractName: string): string;
export declare function packageJsonTemplate(projectName: string): object;
export declare function tsconfigTemplate(): object;
export declare function gitignoreTemplate(): string;
export declare function valuesAwsTemplate(a: InitAnswers): string;
export declare function secretsExampleTemplate(a: InitAnswers): string;
export declare function readmeTemplate(projectName: string): string;
export declare const ERC20_ABI: {
    anonymous: boolean;
    inputs: {
        indexed: boolean;
        name: string;
        type: string;
    }[];
    name: string;
    type: string;
}[];
