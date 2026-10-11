use super::host_state::HostState;
use wasmtime::Caller;

/// Helper to read a UTF-8 string from guest linear memory.
pub fn read_memory_string(
    caller: &mut Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Option<String> {
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
pub fn write_memory_bytes(caller: &mut Caller<'_, HostState>, ptr: i32, bytes: &[u8]) -> bool {
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
