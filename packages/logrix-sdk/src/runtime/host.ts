// Host function declarations and low-level API bindings for Logrix runtime.

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

@external("env", "logrix_db_remove")
declare function host_db_remove(
  key_ptr: i32,
  key_len: i32
): i32;

@external("env", "logrix_log")
declare function host_log(level: i32, msg_ptr: i32, msg_len: i32): i32;

/**
 * Memory allocator export required by the host environment for passing buffers.
 */
export function allocate(size: i32): i32 {
  return heap.alloc(size) as i32;
}

/**
 * Emits an entity modification payload to the host transaction buffer.
 */
export function logrix_emit(entityType: string, jsonPayload: string): void {
  const eBuf = String.UTF8.encode(entityType);
  const pBuf = String.UTF8.encode(jsonPayload);
  const code = host_emit(
    changetype<i32>(eBuf),
    eBuf.byteLength,
    changetype<i32>(pBuf),
    pBuf.byteLength
  );
  if (code < 0) {
    unreachable();
  }
}

/**
 * Retrieves a state value by key from the host key-value store.
 */
export function logrix_db_get(key: string): string {
  const kBuf = String.UTF8.encode(key);
  let bufSize: i32 = 8192;
  let outBuf = new ArrayBuffer(bufSize);
  let written = host_db_get(
    changetype<i32>(kBuf),
    kBuf.byteLength,
    changetype<i32>(outBuf),
    bufSize
  );

  // If buffer is too small (-2), expand dynamically
  while (written == -2 && bufSize <= 1048576) {
    bufSize *= 4;
    outBuf = new ArrayBuffer(bufSize);
    written = host_db_get(
      changetype<i32>(kBuf),
      kBuf.byteLength,
      changetype<i32>(outBuf),
      bufSize
    );
  }

  if (written == -1) return "";
  if (written < 0) {
    unreachable();
  }
  return String.UTF8.decode(outBuf.slice(0, written));
}

/**
 * Sets a state key-value pair in the host store.
 */
export function logrix_db_set(key: string, value: string): void {
  const kBuf = String.UTF8.encode(key);
  const vBuf = String.UTF8.encode(value);
  const code = host_db_set(
    changetype<i32>(kBuf),
    kBuf.byteLength,
    changetype<i32>(vBuf),
    vBuf.byteLength
  );
  if (code < 0) {
    unreachable();
  }
}

/**
 * Removes a state key from the host store.
 */
export function logrix_db_remove(key: string): void {
  const kBuf = String.UTF8.encode(key);
  const code = host_db_remove(changetype<i32>(kBuf), kBuf.byteLength);
  if (code < 0) {
    unreachable();
  }
}

/**
 * Sends a structured log message to the host tracing subscriber.
 */
export function logrix_log(level: i32, message: string): void {
  const mBuf = String.UTF8.encode(message);
  host_log(level, changetype<i32>(mBuf), mBuf.byteLength);
}

/**
 * Store namespace for Graph Protocol compatible entity management.
 */
export namespace store {
  export function get(entityType: string, id: string): string {
    return logrix_db_get(entityType + ":" + id);
  }

  export function set(entityType: string, id: string, jsonPayload: string): void {
    logrix_db_set(entityType + ":" + id, jsonPayload);
    logrix_emit(entityType, jsonPayload);
  }

  export function remove(entityType: string, id: string): void {
    logrix_db_remove(entityType + ":" + id);
    logrix_emit(entityType, "{\"id\":\"" + id + "\",\"_deleted\":true}");
  }
}

/**
 * Log namespace for Graph Protocol compatible logging.
 */
export namespace log {
  export function debug(message: string): void {
    logrix_log(0, message);
  }

  export function info(message: string): void {
    logrix_log(1, message);
  }

  export function warning(message: string): void {
    logrix_log(2, message);
  }

  export function error(message: string): void {
    logrix_log(3, message);
  }
}
