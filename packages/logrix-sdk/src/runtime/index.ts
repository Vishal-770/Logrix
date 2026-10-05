// AssemblyScript runtime library for Logrix indexer handlers.
// Import this in your handler files: import { logrix_emit, logrix_db_get, ... } from "@logrix/sdk"

// ---------------------------------------------------------------------------
// Host function declarations
// The Logrix Rust engine registers these in both "env" and "logrix" namespaces.
// ---------------------------------------------------------------------------

@external("env", "logrix_emit")
declare function host_emit(
  entity_ptr: i32,
  entity_len: i32,
  val_ptr: i32,
  val_len: i32
): i32;

@external("env", "logrix_db_get")
declare function host_db_get(
  key_ptr: i32,
  key_len: i32,
  out_ptr: i32,
  max_out_len: i32
): i32;

@external("env", "logrix_db_set")
declare function host_db_set(
  key_ptr: i32,
  key_len: i32,
  val_ptr: i32,
  val_len: i32
): i32;

@external("env", "logrix_log")
declare function host_log(level: i32, msg_ptr: i32, msg_len: i32): i32;

// ---------------------------------------------------------------------------
// Memory allocator export
// The Rust engine calls allocate(len: i32) -> i32 to write event payloads
// into guest memory before invoking handle_event.
// ---------------------------------------------------------------------------

export function allocate(size: i32): i32 {
  return heap.alloc(size) as i32;
}

// ---------------------------------------------------------------------------
// Raw blockchain event log — mirrors the JSON written by the Rust engine.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Emit a structured entity to PostgreSQL / GraphQL.
 * @param entityType Entity type name matching your schema.graphql
 * @param jsonPayload Serialized JSON string of the entity fields
 */
export function logrix_emit(entityType: string, jsonPayload: string): void {
  const eBuf = String.UTF8.encode(entityType);
  const pBuf = String.UTF8.encode(jsonPayload);
  host_emit(
    changetype<i32>(eBuf),
    eBuf.byteLength,
    changetype<i32>(pBuf),
    pBuf.byteLength
  );
}

/**
 * Retrieve a persisted value by key (cross-block state).
 * Returns empty string if the key does not exist.
 */
export function logrix_db_get(key: string): string {
  const kBuf = String.UTF8.encode(key);
  const outBuf = new ArrayBuffer(4096);
  const written = host_db_get(
    changetype<i32>(kBuf),
    kBuf.byteLength,
    changetype<i32>(outBuf),
    outBuf.byteLength
  );
  if (written <= 0) return "";
  return String.UTF8.decode(outBuf.slice(0, written));
}

/**
 * Persist a value by key (cross-block state).
 */
export function logrix_db_set(key: string, value: string): void {
  const kBuf = String.UTF8.encode(key);
  const vBuf = String.UTF8.encode(value);
  host_db_set(
    changetype<i32>(kBuf),
    kBuf.byteLength,
    changetype<i32>(vBuf),
    vBuf.byteLength
  );
}

/**
 * Log a diagnostic message.
 * @param level 0=DEBUG 1=INFO 2=WARN 3=ERROR
 */
export function logrix_log(level: i32, message: string): void {
  const mBuf = String.UTF8.encode(message);
  host_log(level, changetype<i32>(mBuf), mBuf.byteLength);
}
