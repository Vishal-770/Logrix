// External WebAssembly Host Bindings provided by the Logrix Rust Engine

@external("env", "logrix_emit")
declare function host_emit(
  entity_ptr: usize,
  entity_len: usize,
  val_ptr: usize,
  val_len: usize
): i32;

@external("env", "logrix_db_get")
declare function host_db_get(
  key_ptr: usize,
  key_len: usize,
  out_ptr: usize,
  max_out_len: usize
): i32;

@external("env", "logrix_db_set")
declare function host_db_set(
  key_ptr: usize,
  key_len: usize,
  val_ptr: usize,
  val_len: usize
): i32;

@external("env", "logrix_log")
declare function host_log(level: i32, msg_ptr: usize, msg_len: usize): i32;

/**
 * Raw blockchain event log payload delivered to indexer handlers.
 */
export class EventLog {
  address: string = "";
  block_number: u64 = 0;
  block_hash: string = "";
  transaction_hash: string = "";
  log_index: u32 = 0;
  block_timestamp: u64 = 0;
  topics: Array<string> = [];
  data: string = "";
}

/**
 * Emit a structured entity to be written into PostgreSQL and exposed via GraphQL.
 * @param entityType The entity type name matching your schema.graphql
 * @param jsonPayload Serialized JSON string of the entity fields
 */
export function logrix_emit(entityType: string, jsonPayload: string): void {
  let eBuf = String.UTF8.encode(entityType);
  let pBuf = String.UTF8.encode(jsonPayload);
  host_emit(
    changetype<usize>(eBuf),
    eBuf.byteLength,
    changetype<usize>(pBuf),
    pBuf.byteLength
  );
}

/**
 * Retrieve a persisted state value by key across blocks.
 * @param key Unique key to query
 * @returns Stored string value or empty string if not found
 */
export function logrix_db_get(key: string): string {
  let kBuf = String.UTF8.encode(key);
  let outBuf = new ArrayBuffer(4096);
  let written = host_db_get(
    changetype<usize>(kBuf),
    kBuf.byteLength,
    changetype<usize>(outBuf),
    outBuf.byteLength
  );
  if (written <= 0) return "";
  return String.UTF8.decode(outBuf.slice(0, written));
}

/**
 * Persist a state value by key across blocks.
 * @param key Unique key to save
 * @param value String value to store
 */
export function logrix_db_set(key: string, value: string): void {
  let kBuf = String.UTF8.encode(key);
  let vBuf = String.UTF8.encode(value);
  host_db_set(
    changetype<usize>(kBuf),
    kBuf.byteLength,
    changetype<usize>(vBuf),
    vBuf.byteLength
  );
}

/**
 * Log a diagnostic message with severity level.
 * @param level 0=DEBUG, 1=INFO, 2=WARN, 3=ERROR
 * @param message Message to log
 */
export function logrix_log(level: i32, message: string): void {
  let mBuf = String.UTF8.encode(message);
  host_log(level, changetype<usize>(mBuf), mBuf.byteLength);
}

