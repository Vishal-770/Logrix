// External WebAssembly Host Bindings provided by the Logrix Rust Engine

@external("env", "logrix_emit")
declare function host_emit(ptr: usize, len: usize): void;

@external("env", "logrix_db_get")
declare function host_db_get(ptr: usize, len: usize): usize;

@external("env", "logrix_db_set")
declare function host_db_set(k_ptr: usize, k_len: usize, v_ptr: usize, v_len: usize): void;

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
  let combined = entityType + "|" + jsonPayload;
  let buffer = String.UTF8.encode(combined);
  host_emit(changetype<usize>(buffer), buffer.byteLength);
}

/**
 * Retrieve a persisted state value by key across blocks.
 * @param key Unique key to query
 * @returns Stored string value or empty string if not found
 */
export function logrix_db_get(key: string): string {
  let buffer = String.UTF8.encode(key);
  let resPtr = host_db_get(changetype<usize>(buffer), buffer.byteLength);
  if (resPtr == 0) return "";
  return String.UTF8.decode(changetype<ArrayBuffer>(resPtr));
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
