//! # Data Masking
//!
//! Utilities for masking sensitive data in logs, displays, and serialized output.
//!
//! Provides:
//! - [`SensitiveData`] wrapper that auto-masks on `Display` / `Serialize`
//! - Standalone masking helpers: [`mask_string`], [`mask_email`], [`mask_ip`], [`mask_token`]
//! - [`redact_log_message`] to auto-detect and mask emails, IPs, and tokens in free text

use serde::{Serialize, Serializer};
use std::fmt;

// ── Standalone masking helpers ────────────────────────────────────────

/// Mask a string, showing only the first `visible_chars` characters followed
/// by `***`.  If the string is shorter than `visible_chars`, the entire
/// string is replaced with `***`.
///
/// ```text
/// mask_string("abcdefgh", 3) → "abc***"
/// mask_string("ab", 3)       → "***"
/// ```
pub fn mask_string(s: &str, visible_chars: usize) -> String {
    if s.len() <= visible_chars {
        return "***".to_string();
    }
    let visible = &s[..visible_chars];
    format!("{visible}***")
}

/// Mask an email address, keeping the first two characters of the local part
/// and the full domain.
///
/// ```text
/// mask_email("john.doe@example.com") → "jo***@example.com"
/// ```
pub fn mask_email(email: &str) -> String {
    match email.find('@') {
        Some(pos) if pos >= 2 => {
            let local = &email[..2];
            let domain = &email[pos..];
            format!("{local}***{domain}")
        }
        Some(pos) => {
            // local part is 0 or 1 chars
            let local = &email[..pos];
            let domain = &email[pos..];
            format!("{local}***{domain}")
        }
        None => mask_string(email, 2),
    }
}

/// Mask an IPv4 address, replacing the last octet with `***`.
///
/// ```text
/// mask_ip("192.168.1.100") → "192.168.1.***"
/// ```
pub fn mask_ip(ip: &str) -> String {
    if let Some(last_dot) = ip.rfind('.') {
        format!("{}.***", &ip[..last_dot])
    } else {
        "***".to_string()
    }
}

/// Mask a token / secret, showing only the first 8 characters.
///
/// ```text
/// mask_token("eyJhbGciOiJIUzI1NiJ9.signature") → "eyJhbGci***"
/// ```
pub fn mask_token(token: &str) -> String {
    mask_string(token, 8)
}

/// Auto-detect and mask emails, IPv4 addresses, and bearer-style tokens
/// inside a free-text log message.
///
/// Detection rules:
/// - **Email**: `\S+@\S+\.\S+` → masked via [`mask_email`]
/// - **IPv4**:  `\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}` → masked via [`mask_ip`]
/// - **Token**: long hex/base64-like strings ≥ 32 chars (word-boundary
///   delimited) → masked via [`mask_token`]
pub fn redact_log_message(msg: &str) -> String {
    let mut result = msg.to_string();

    // 1. Mask emails  (simple heuristic: word chars around @)
    result = redact_emails(&result);

    // 2. Mask IPv4 addresses
    result = redact_ips(&result);

    // 3. Mask long tokens (hex or base64-like strings ≥ 32 chars)
    result = redact_tokens(&result);

    result
}

/// Internal: find and mask email-like patterns.
fn redact_emails(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for an '@' preceded and followed by non-space chars
        if bytes[i] == b'@' && i > 0 && i + 1 < len {
            // Find start of local part (walk backwards)
            let start = {
                let mut j = i;
                while j > 0 && !bytes[j - 1].is_ascii_whitespace() {
                    j -= 1;
                }
                j
            };
            // Find end of domain (walk forwards)
            let end = {
                let mut j = i + 1;
                while j < len && !bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                j
            };
            if start < i && end > i + 1 {
                let email = &s[start..end];
                out.push_str(&mask_email(email));
                i = end;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Internal: find and mask IPv4 addresses.
fn redact_ips(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Try to match an IPv4 pattern starting at position i
        if chars[i].is_ascii_digit() {
            if let Some(end) = try_match_ipv4(&chars, i) {
                let ip: String = chars[i..end].iter().collect();
                out.push_str(&mask_ip(&ip));
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Try to match `\d{1,3}.\d{1,3}.\d{1,3}.\d{1,3}` starting at `start`.
/// Returns the end index (exclusive) on success.
fn try_match_ipv4(chars: &[char], start: usize) -> Option<usize> {
    let len = chars.len();
    let mut pos = start;

    for octet_idx in 0..4 {
        // Parse 1-3 digits
        let digit_start = pos;
        while pos < len && chars[pos].is_ascii_digit() && pos - digit_start < 3 {
            pos += 1;
        }
        if pos == digit_start {
            return None; // no digits
        }
        // Check octet value ≤ 255
        let octet_str: String = chars[digit_start..pos].iter().collect();
        if octet_str.parse::<u8>().is_err() {
            return None;
        }
        // Expect '.' between octets (not after the last one)
        if octet_idx < 3 {
            if pos >= len || chars[pos] != '.' {
                return None;
            }
            pos += 1; // skip the dot
        }
    }

    // Ensure the match ends at a word boundary (not followed by another digit or letter)
    if pos < len && (chars[pos].is_ascii_alphanumeric()) {
        return None;
    }

    Some(pos)
}

/// Internal: find and mask long token-like strings (hex or base64 ≥ 32 chars).
fn redact_tokens(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if is_token_char(bytes[i]) {
            let start = i;
            while i < len && is_token_char(bytes[i]) {
                i += 1;
            }
            let token_len = i - start;
            if token_len >= 32 {
                let token = &s[start..i];
                out.push_str(&mask_token(token));
            } else {
                out.push_str(&s[start..i]);
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Characters that can appear in a token (hex digits, base64 chars, dots, dashes).
fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_'
}

// ── SensitiveData wrapper ─────────────────────────────────────────────

/// A wrapper for sensitive data that automatically masks its value when
/// displayed (via [`fmt::Display`]) or serialized (via [`serde::Serialize`]).
///
/// The inner value is accessible via [`SensitiveData::expose`] for
/// legitimate processing.
#[derive(Debug, Clone)]
pub struct SensitiveData {
    inner: String,
    kind: SensitiveKind,
}

#[derive(Debug, Clone, Copy)]
enum SensitiveKind {
    Generic,
    Email,
    Ip,
    Token,
}

impl SensitiveData {
    /// Wrap a generic sensitive string.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            inner: value.into(),
            kind: SensitiveKind::Generic,
        }
    }

    /// Wrap a sensitive email address.
    pub fn email(value: impl Into<String>) -> Self {
        Self {
            inner: value.into(),
            kind: SensitiveKind::Email,
        }
    }

    /// Wrap a sensitive IP address.
    pub fn ip(value: impl Into<String>) -> Self {
        Self {
            inner: value.into(),
            kind: SensitiveKind::Ip,
        }
    }

    /// Wrap a sensitive token / secret.
    pub fn token(value: impl Into<String>) -> Self {
        Self {
            inner: value.into(),
            kind: SensitiveKind::Token,
        }
    }

    /// Access the raw (unmasked) value.  Use only for legitimate processing.
    pub fn expose(&self) -> &str {
        &self.inner
    }

    /// Return the masked representation as a `String`.
    pub fn masked(&self) -> String {
        match self.kind {
            SensitiveKind::Generic => mask_string(&self.inner, 3),
            SensitiveKind::Email => mask_email(&self.inner),
            SensitiveKind::Ip => mask_ip(&self.inner),
            SensitiveKind::Token => mask_token(&self.inner),
        }
    }
}

impl fmt::Display for SensitiveData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.masked())
    }
}

impl Serialize for SensitiveData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.masked())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_string_basic() {
        assert_eq!(mask_string("abcdefgh", 3), "abc***");
    }

    #[test]
    fn test_mask_string_short_input() {
        assert_eq!(mask_string("ab", 3), "***");
    }

    #[test]
    fn test_mask_string_exact_length() {
        assert_eq!(mask_string("abc", 3), "***");
    }

    #[test]
    fn test_mask_email_standard() {
        assert_eq!(mask_email("john.doe@example.com"), "jo***@example.com");
    }

    #[test]
    fn test_mask_email_short_local() {
        assert_eq!(mask_email("a@b.com"), "a***@b.com");
    }

    #[test]
    fn test_mask_email_no_at() {
        // No '@' → falls back to generic masking
        assert_eq!(mask_email("notanemail"), "no***");
    }

    #[test]
    fn test_mask_ip_standard() {
        assert_eq!(mask_ip("192.168.1.100"), "192.168.1.***");
    }

    #[test]
    fn test_mask_ip_no_dots() {
        assert_eq!(mask_ip("invalid"), "***");
    }

    #[test]
    fn test_mask_token_standard() {
        let long_token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0";
        assert_eq!(mask_token(long_token), "eyJhbGci***");
    }

    #[test]
    fn test_mask_token_short() {
        assert_eq!(mask_token("short"), "***");
    }

    #[test]
    fn test_redact_log_message_email() {
        let msg = "User john@example.com logged in";
        let redacted = redact_log_message(msg);
        assert!(redacted.contains("jo***@example.com"));
        assert!(!redacted.contains("john@example.com"));
    }

    #[test]
    fn test_redact_log_message_ip() {
        let msg = "Request from 10.0.0.42 received";
        let redacted = redact_log_message(msg);
        assert!(redacted.contains("10.0.0.***"));
        assert!(!redacted.contains("10.0.0.42"));
    }

    #[test]
    fn test_redact_log_message_long_token() {
        let token = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8";
        let msg = format!("Token {token} used");
        let redacted = redact_log_message(&msg);
        assert!(redacted.contains("a1b2c3d4***"));
        assert!(!redacted.contains(token));
    }

    #[test]
    fn test_redact_log_message_combined() {
        let msg = "User admin@test.org connected from 172.16.0.5 with token abcdef0123456789abcdef0123456789abcdef01";
        let redacted = redact_log_message(msg);
        assert!(redacted.contains("ad***@test.org"));
        assert!(redacted.contains("172.16.0.***"));
        assert!(redacted.contains("abcdef01***"));
    }

    #[test]
    fn test_sensitive_data_display_generic() {
        let sd = SensitiveData::new("supersecret");
        assert_eq!(format!("{sd}"), "sup***");
    }

    #[test]
    fn test_sensitive_data_display_email() {
        let sd = SensitiveData::email("user@domain.com");
        assert_eq!(format!("{sd}"), "us***@domain.com");
    }

    #[test]
    fn test_sensitive_data_display_ip() {
        let sd = SensitiveData::ip("10.0.0.1");
        assert_eq!(format!("{sd}"), "10.0.0.***");
    }

    #[test]
    fn test_sensitive_data_display_token() {
        let sd = SensitiveData::token("abcdefghijklmnop");
        assert_eq!(format!("{sd}"), "abcdefgh***");
    }

    #[test]
    fn test_sensitive_data_expose() {
        let sd = SensitiveData::new("rawvalue");
        assert_eq!(sd.expose(), "rawvalue");
    }

    #[test]
    fn test_sensitive_data_serialize() {
        let sd = SensitiveData::email("test@example.com");
        let json = serde_json::to_value(&sd).unwrap();
        assert_eq!(json.as_str().unwrap(), "te***@example.com");
    }

    #[test]
    fn test_redact_log_message_no_sensitive_data() {
        let msg = "Nothing sensitive here";
        assert_eq!(redact_log_message(msg), msg);
    }
}
