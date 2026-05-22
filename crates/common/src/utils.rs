//! General-purpose utility functions for KIAS.

use chrono::Utc;
use sha2::{Digest, Sha256};

// ── Hashing ───────────────────────────────────────────────────────────

/// Compute a SHA-256 hex digest of the input bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(&hasher.finalize())
}

/// Compute a Blake3 hex digest of the input bytes (faster than SHA-256).
pub fn blake3_hex(data: &[u8]) -> String {
    let hash = blake3::hash(data);
    hash.to_hex().to_string()
}

/// Hash a "prefix" string (e.g. a system prompt) – the canonical helper
/// used by cache-hub and scheduler.
pub fn hash_prefix(prefix: &str) -> u64 {
    let hash = blake3::hash(prefix.as_bytes());
    u64::from_be_bytes(hash.as_bytes()[..8].try_into().expect("slice len"))
}

// ── Time helpers ──────────────────────────────────────────────────────

/// Return the current UTC time as an ISO-8601 string.
pub fn now_utc_iso8601() -> String {
    Utc::now().to_rfc3339()
}

/// Return the current Unix timestamp in seconds.
pub fn now_unix_secs() -> i64 {
    Utc::now().timestamp()
}

/// Return the current Unix timestamp in milliseconds.
pub fn now_unix_millis() -> i64 {
    Utc::now().timestamp_millis()
}

// ── ID generation ─────────────────────────────────────────────────────

/// Generate a new v4 UUID string (no hyphens, 32 hex chars).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Generate a new v4 UUID with the given prefix, e.g. `agent-a1b2c3...`.
pub fn new_prefixed_id(prefix: &str) -> String {
    let short = &uuid::Uuid::new_v4().to_string()[..8];
    format!("{prefix}-{short}")
}

// ── Misc ──────────────────────────────────────────────────────────────

/// Encode bytes as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Clamp a value to `[lo, hi]`.
pub fn clamp<T: Ord>(value: T, lo: T, hi: T) -> T {
    if value < lo {
        lo
    } else if value > hi {
        hi
    } else {
        value
    }
}

/// Convert a percentage (0-100) to a float ratio (0.0-1.0).
pub fn percent_to_ratio(percent: u8) -> f64 {
    (percent.min(100) as f64) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex_deterministic() {
        let a = sha256_hex(b"hello");
        let b = sha256_hex(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn test_blake3_hex_deterministic() {
        let a = blake3_hex(b"hello");
        let b = blake3_hex(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn test_hash_prefix_deterministic() {
        let a = hash_prefix("system: you are a helpful assistant");
        let b = hash_prefix("system: you are a helpful assistant");
        assert_eq!(a, b);
    }

    #[test]
    fn test_now_utc_iso8601_not_empty() {
        let s = now_utc_iso8601();
        assert!(s.contains('T'));
    }

    #[test]
    fn test_new_id_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
    }

    #[test]
    fn test_new_prefixed_id() {
        let id = new_prefixed_id("agent");
        assert!(id.starts_with("agent-"));
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-1, 0, 10), 0);
        assert_eq!(clamp(15, 0, 10), 10);
    }

    #[test]
    fn test_percent_to_ratio() {
        assert!((percent_to_ratio(50) - 0.5).abs() < f64::EPSILON);
        assert!((percent_to_ratio(100) - 1.0).abs() < f64::EPSILON);
        assert!((percent_to_ratio(200) - 1.0).abs() < f64::EPSILON); // clamped
    }
}
