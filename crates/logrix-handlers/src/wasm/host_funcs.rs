use super::host_state::HostState;
use super::memory::{read_memory_string, write_memory_bytes};
use wasmtime::{Caller, Linker};

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
                    Err(_) => return -3, // Invalid JSON payload
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

    linker.func_wrap(
        "env",
        "abort",
        |mut caller: Caller<'_, HostState>, msg_ptr: i32, file_ptr: i32, line: i32, col: i32| {
            let msg = read_memory_string(&mut caller, msg_ptr, 128)
                .unwrap_or_else(|| "WASM abort called".to_string());
            let file = read_memory_string(&mut caller, file_ptr, 64)
                .unwrap_or_else(|| "unknown".to_string());
            caller
                .data_mut()
                .staging
                .log("ERROR".to_string(), format!("{msg} at {file}:{line}:{col}"));
        },
    )?;

    Ok(())
}
