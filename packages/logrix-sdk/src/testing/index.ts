import { EventLog } from "../runtime";

export interface MockEventLogConfig {
  address?: string;
  blockNumber?: number;
  blockHash?: string;
  transactionHash?: string;
  logIndex?: number;
  blockTimestamp?: number;
  topics?: string[];
  data?: string;
}

/**
 * Construct an EventLog instance pre-populated with test values.
 * Useful for writing local unit tests for AssemblyScript/TypeScript mapping handlers.
 */
export function createMockEventLog(config: MockEventLogConfig = {}): EventLog {
  const log = new EventLog();
  log.address = config.address || "0x0000000000000000000000000000000000000000";
  log.block_number = BigInt(config.blockNumber ?? 1) as any;
  log.block_hash = config.blockHash || "0x0000000000000000000000000000000000000000000000000000000000000000";
  log.transaction_hash = config.transactionHash || "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
  log.log_index = (config.logIndex ?? 0) as any;
  log.block_timestamp = BigInt(config.blockTimestamp ?? 1700000000) as any;
  log.topics = config.topics || [];
  log.data = config.data || "0x";
  return log;
}
