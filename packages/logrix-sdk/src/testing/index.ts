import { EventLog } from "../runtime/index";

export class MockEventLogConfig {
  address: string = "0x0000000000000000000000000000000000000000";
  blockNumber: u64 = 1;
  blockHash: string = "0x0000000000000000000000000000000000000000000000000000000000000000";
  transactionHash: string = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
  logIndex: u64 = 0;
  blockTimestamp: u64 = 1700000000;
  topics: string[] = [];
  data: string = "0x";
}

/**
 * Construct an EventLog instance pre-populated with test values in AssemblyScript.
 */
export function createMockEventLog(config: MockEventLogConfig | null = null): EventLog {
  const log = new EventLog();
  if (config != null) {
    log.address = config.address;
    log.block_number = config.blockNumber;
    log.block_hash = config.blockHash;
    log.transaction_hash = config.transactionHash;
    log.log_index = config.logIndex;
    log.block_timestamp = config.blockTimestamp;
    log.topics = config.topics;
    log.data = config.data;
  } else {
    log.address = "0x0000000000000000000000000000000000000000";
    log.block_number = 1;
    log.block_hash = "0x0000000000000000000000000000000000000000000000000000000000000000";
    log.transaction_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    log.log_index = 0;
    log.block_timestamp = 1700000000;
    log.topics = [];
    log.data = "0x";
  }
  return log;
}
