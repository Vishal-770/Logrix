use super::host_state::HostState;
use wasmtime::{Caller, Linker};

/// Helper to read a UTF-8 string from guest linear memory.
fn read_memory_string(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Option<String> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(caller);
    let start = ptr as usize;
    let end = start.checked_add(len as usize)?;
    if end <= data.len() {
        String::from_utf8(data[start..end].to_vec()).ok()
    } else {
        None
    }
}

/// Helper to write bytes into guest linear memory.
fn write_memory_bytes(caller: &mut Caller<'_, HostState>, ptr: i32, bytes: &[u8]) -> bool {
    if ptr < 0 {
        return false;
    }
    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return false,
    };
    let start = ptr as usize;
    let end = match start.checked_add(bytes.len()) {
        Some(e) => e,
        None => return false,
    };
    let data = memory.data_mut(caller);
    if end <= data.len() {
        data[start..end].copy_from_slice(bytes);
        true
    } else {
        false
    }
}

/// Register standard host functions into the Wasmtime Linker under both "logrix" and "env".
pub fn register_host_functions(linker: &mut Linker<HostState>) -> Result<(), wasmtime::Error> {
    for namespace in &["logrix", "env"] {
        // logrix_db_get(key_ptr, key_len, out_ptr, max_out_len) -> i32 (bytes written, -1 not found, -2 buf too small)
        linker.func_wrap(
            namespace,
            "logrix_db_get",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             out_ptr: i32,
             max_out_len: i32|
             -> i32 {
                let key = match read_memory_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return -1,
                };

                let val_opt = caller.data().get_value(&key).cloned();
                match val_opt {
                    Some(val) => {
                        let bytes = val.as_bytes();
                        if bytes.len() <= max_out_len as usize {
                            if write_memory_bytes(&mut caller, out_ptr, bytes) {
                                bytes.len() as i32
                            } else {
                                -2
                            }
                        } else {
                            -2 // Output buffer too small
                        }
                    }
                    None => -1, // Key not found in staging or base snapshot
                }
            },
        )?;

        // logrix_db_set(key_ptr, key_len, val_ptr, val_len) -> i32
        linker.func_wrap(
            namespace,
            "logrix_db_set",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             val_ptr: i32,
             val_len: i32|
             -> i32 {
                let key = match read_memory_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return -1,
                };
                let val = match read_memory_string(&mut caller, val_ptr, val_len) {
                    Some(v) => v,
                    None => return -1,
                };

                caller.data_mut().staging.set_state(key, val);
                0
            },
        )?;

        // logrix_db_remove(key_ptr, key_len) -> i32
        // Stages a key removal; the engine propagates this as a soft-delete after commit.
        linker.func_wrap(
            namespace,
            "logrix_db_remove",
            |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
                let key = match read_memory_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return -1,
                };
                caller.data_mut().staging.remove_state(key);
                0
            },
        )?;

        // logrix_emit(entity_ptr, entity_len, val_ptr, val_len) -> i32
        linker.func_wrap(
            namespace,
            "logrix_emit",
            |mut caller: Caller<'_, HostState>,
             entity_ptr: i32,
             entity_len: i32,
             val_ptr: i32,
             val_len: i32|
             -> i32 {
                let entity = match read_memory_string(&mut caller, entity_ptr, entity_len) {
                    Some(e) => e,
                    None => return -1,
                };
                let val_str = match read_memory_string(&mut caller, val_ptr, val_len) {
                    Some(v) => v,
                    None => return -1,
                };

                let payload = match serde_json::from_str(&val_str) {
                    Ok(p) => p,
                    Err(_) => serde_json::Value::String(val_str),
                };

                caller.data_mut().staging.emit_entity(entity, payload);
                0
            },
        )?;

        // logrix_log(level, msg_ptr, msg_len) -> i32
        linker.func_wrap(
            namespace,
            "logrix_log",
            |mut caller: Caller<'_, HostState>, level: i32, msg_ptr: i32, msg_len: i32| -> i32 {
                let msg = match read_memory_string(&mut caller, msg_ptr, msg_len) {
                    Some(m) => m,
                    None => return -1,
                };
                let lvl = match level {
                    0 => "DEBUG",
                    1 => "INFO",
                    2 => "WARN",
                    _ => "ERROR",
                };
                caller.data_mut().staging.log(lvl.to_string(), msg);
                0
            },
        )?;
    }

    Ok(())
}
