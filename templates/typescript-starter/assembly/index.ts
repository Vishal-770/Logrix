// Logrix WebAssembly TypeScript / AssemblyScript Guest Handler SDK

@external("logrix", "logrix_db_get")
declare function host_db_get(key_ptr: usize, key_len: usize, out_ptr: usize, max_len: usize): i32;

@external("logrix", "logrix_db_set")
declare function host_db_set(key_ptr: usize, key_len: usize, val_ptr: usize, val_len: usize): i32;

@external("logrix", "logrix_emit")
declare function host_emit(entity_ptr: usize, entity_len: usize, val_ptr: usize, val_len: usize): i32;

@external("logrix", "logrix_log")
declare function host_log(level: i32, msg_ptr: usize, msg_len: usize): i32;

/**
 * Public memory allocation hook used by Logrix host when feeding event buffers.
 */
export function allocate(size: usize): usize {
  return heap.alloc(size);
}

/**
 * Main event handler invoked by Logrix for each matching contract event log.
 */
export function handle_event(ptr: usize, len: usize): i32 {
  // Read event input string from linear memory
  let eventJson = String.UTF8.decode(ptr, len);

  // Example: Query current state balance
  let key = "account_balance:0xUSDC";
  let keyUtf8 = String.UTF8.encode(key);
  let buffer = new ArrayBuffer(64);
  let readBytes = host_db_get(
    changetype<usize>(keyUtf8),
    keyUtf8.byteLength,
    changetype<usize>(buffer),
    64
  );

  let newBalance: i64 = 1;
  if (readBytes > 0) {
    let currentValStr = String.UTF8.decode(changetype<usize>(buffer), readBytes);
    newBalance = I64.parseInt(currentValStr) + 1;
  }

  // Save updated balance to host state
  let newValStr = newBalance.toString();
  let valUtf8 = String.UTF8.encode(newValStr);
  host_db_set(
    changetype<usize>(keyUtf8),
    keyUtf8.byteLength,
    changetype<usize>(valUtf8),
    valUtf8.byteLength
  );

  // Emit structured entity for GraphQL indexing
  let entityName = "AccountBalance";
  let entityUtf8 = String.UTF8.encode(entityName);
  let entityPayload = "{\"balance\":" + newValStr + "}";
  let payloadUtf8 = String.UTF8.encode(entityPayload);
  host_emit(
    changetype<usize>(entityUtf8),
    entityUtf8.byteLength,
    changetype<usize>(payloadUtf8),
    payloadUtf8.byteLength
  );

  // Diagnostic log
  let msg = "Handled event for balance increment";
  let msgUtf8 = String.UTF8.encode(msg);
  host_log(1, changetype<usize>(msgUtf8), msgUtf8.byteLength);

  return 0; // Success
}
