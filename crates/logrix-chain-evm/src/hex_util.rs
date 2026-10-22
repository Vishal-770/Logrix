use alloy_primitives::B256;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};

/// Parse a required 32-byte hash string, rejecting missing or invalid values.
pub fn parse_required_hash(val: Option<&str>, field: &str) -> LogrixResult<B256> {
    let raw = val.ok_or_else(|| {
        LogrixError::new(
            ErrorClass::Permanent,
            ErrorSource::ChainRpc,
            format!("Block response missing required field '{field}'"),
        )
    })?;

    raw.parse::<B256>().map_err(|e| {
        LogrixError::new(
            ErrorClass::Permanent,
            ErrorSource::ChainRpc,
            format!("Invalid hash for field '{field}' ('{raw}'): {e}"),
        )
    })
}

/// Parse a required hex u64 string, returning explicit error on failure.
pub fn parse_required_hex_u64(val: Option<&str>, field: &str) -> LogrixResult<u64> {
    let raw = val.ok_or_else(|| {
        LogrixError::integrity(
            ErrorSource::ChainRpc,
            format!("Missing required hex field '{field}'"),
        )
    })?;

    let clean = raw.trim_start_matches("0x");
    u64::from_str_radix(clean, 16).map_err(|e| {
        LogrixError::integrity(
            ErrorSource::ChainRpc,
            format!("Invalid hex for field '{field}' ('{raw}'): {e}"),
        )
    })
}

/// Parse a hex u64 string with optional 0x prefix, defaulting to 0 if absent or invalid.
pub fn parse_hex_u64(val: Option<&str>) -> u64 {
    val.and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0)
}
