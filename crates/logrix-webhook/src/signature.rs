use hmac::{Hmac, Mac};
use rand::{distributions::Alphanumeric, Rng};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Generate a cryptographically random webhook signing secret (prefixed with `whsec_`).
#[must_use]
pub fn generate_secret() -> String {
    let rand_str: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    format!("whsec_{rand_str}")
}

/// Sign a webhook payload using HMAC-SHA256.
///
/// Output format: `t={timestamp},v1={hex_digest}`
#[must_use]
pub fn sign_payload(secret: &str, timestamp: u64, payload: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");

    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);

    let signature = hex::encode(mac.finalize().into_bytes());
    format!("t={timestamp},v1={signature}")
}

/// Verify that an incoming `X-Logrix-Signature` header matches the payload and secret.
/// Also rejects signatures older than `tolerance_seconds` to prevent replay attacks.
#[must_use]
pub fn verify_signature(
    secret: &str,
    header: &str,
    payload: &[u8],
    tolerance_seconds: u64,
) -> bool {
    let mut timestamp: Option<u64> = None;
    let mut expected_v1: Option<&str> = None;

    for part in header.split(',') {
        let mut kv = part.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("t"), Some(t_str)) => {
                timestamp = t_str.parse().ok();
            }
            (Some("v1"), Some(sig)) => {
                expected_v1 = Some(sig);
            }
            _ => {}
        }
    }

    let (ts, expected_sig) = match (timestamp, expected_v1) {
        (Some(ts), Some(sig)) => (ts, sig),
        _ => return false,
    };

    // Replay attack prevention: check timestamp freshness
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now.saturating_sub(ts) > tolerance_seconds {
        return false;
    }

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(ts.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);

    let computed_sig = hex::encode(mac.finalize().into_bytes());
    computed_sig.as_bytes() == expected_sig.as_bytes()
}
