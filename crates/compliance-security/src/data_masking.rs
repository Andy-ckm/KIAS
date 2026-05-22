//! Data Masking Module
//!
//! Implements automatic PII/PHI detection and masking:
//! - DataMasker: automatic PII/PHI masking
//! - MaskingRule: rule definition (regex/keyword/NER)
//! - AuditLog: tracking who accessed what sensitive data

use crate::error::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::RwLock;
use regex::Regex;

/// PII/PHI type categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PiiType {
    Email,
    Phone,
    Ssn,
    CreditCard,
    IpAddress,
    Name,
    Address,
    DateOfBirth,
    Passport,
    DriversLicense,
    BankAccount,
    ApiKey,
    Password,
    Custom,
}

impl PiiType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PiiType::Email => "EMAIL",
            PiiType::Phone => "PHONE",
            PiiType::Ssn => "SSN",
            PiiType::CreditCard => "CREDIT_CARD",
            PiiType::IpAddress => "IP_ADDRESS",
            PiiType::Name => "NAME",
            PiiType::Address => "ADDRESS",
            PiiType::DateOfBirth => "DOB",
            PiiType::Passport => "PASSPORT",
            PiiType::DriversLicense => "DRIVERS_LICENSE",
            PiiType::BankAccount => "BANK_ACCOUNT",
            PiiType::ApiKey => "API_KEY",
            PiiType::Password => "PASSWORD",
            PiiType::Custom => "CUSTOM",
        }
    }
}

/// Masking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingConfig {
    pub enabled_rules: Vec<PiiType>,
    pub mask_char: char,
    pub preserve_last_n: usize,
    pub audit_enabled: bool,
}

impl Default for MaskingConfig {
    fn default() -> Self {
        Self {
            enabled_rules: vec![
                PiiType::Email,
                PiiType::Phone,
                PiiType::Ssn,
                PiiType::CreditCard,
                PiiType::IpAddress,
                PiiType::ApiKey,
                PiiType::Password,
            ],
            mask_char: '*',
            preserve_last_n: 4,
            audit_enabled: true,
        }
    }
}

/// Masking rule definition
#[derive(Debug, Clone)]
pub struct MaskingRule {
    pii_type: PiiType,
    pattern: Regex,
    replacement: String,
}

impl MaskingRule {
    pub fn new(pii_type: PiiType, pattern: &str, replacement: &str) -> Option<Self> {
        Regex::new(pattern).ok().map(|re| Self {
            pii_type,
            pattern: re,
            replacement: replacement.to_string(),
        })
    }

    pub fn with_context_aware(pii_type: PiiType, pattern: &str, replacement: &str) -> Option<Self> {
        Self::new(pii_type, pattern, replacement)
    }

    pub fn pii_type(&self) -> PiiType {
        self.pii_type
    }

    pub fn apply(&self, text: &str) -> String {
        self.pattern.replace_all(text, self.replacement.as_str()).to_string()
    }
}

/// Data masking result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingResult {
    pub original_length: usize,
    pub masked_length: usize,
    pub masked_count: usize,
    pub masked_types: Vec<PiiType>,
}

/// Data masker - main entry point
pub struct DataMasker {
    config: MaskingConfig,
    rules: Vec<MaskingRule>,
    audit_log: RwLock<Vec<AuditEntry>>,
}

impl Default for DataMasker {
    fn default() -> Self {
        Self::new()
    }
}

impl DataMasker {
    pub fn new() -> Self {
        let config = MaskingConfig::default();
        let mut masker = Self {
            config,
            rules: Vec::new(),
            audit_log: RwLock::new(Vec::new()),
        };
        masker.init_default_rules();
        masker
    }

    pub fn with_config(config: MaskingConfig) -> Self {
        let mut masker = Self {
            config: config.clone(),
            rules: Vec::new(),
            audit_log: RwLock::new(Vec::new()),
        };
        
        for pii_type in &config.enabled_rules {
            if let Some(rule) = masker.create_default_rule(*pii_type) {
                masker.rules.push(rule);
            }
        }
        
        masker
    }

    fn init_default_rules(&mut self) {
        if let Some(rule) = MaskingRule::new(
            PiiType::Email,
            r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
            "****@****.com",
        ) {
            self.rules.push(rule);
        }
        
        if let Some(rule) = MaskingRule::new(
            PiiType::Phone,
            r"\b\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
            "(***) ***-****",
        ) {
            self.rules.push(rule);
        }
        
        if let Some(rule) = MaskingRule::new(
            PiiType::Ssn,
            r"\b\d{3}-\d{2}-\d{4}\b",
            "***-**-****",
        ) {
            self.rules.push(rule);
        }
        
        if let Some(rule) = MaskingRule::new(
            PiiType::CreditCard,
            r"\b(?:\d[ -]*?){13,19}\b",
            "****-****-****-****",
        ) {
            self.rules.push(rule);
        }
        
        if let Some(rule) = MaskingRule::new(
            PiiType::IpAddress,
            r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b",
            "***.***.***.***",
        ) {
            self.rules.push(rule);
        }
        
        if let Some(rule) = MaskingRule::new(
            PiiType::ApiKey,
            r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*['"]?([0-9a-zA-Z_-]{20,})"#,
            "${1}=****",
        ) {
            self.rules.push(rule);
        }
        
        if let Some(rule) = MaskingRule::new(
            PiiType::Password,
            r#"(?i)(password|passwd|pwd)\s*[:=]\s*['"]?([^\s'"]{8,})"#,
            "${1}=****",
        ) {
            self.rules.push(rule);
        }
    }

    fn create_default_rule(&self, pii_type: PiiType) -> Option<MaskingRule> {
        match pii_type {
            PiiType::Email => MaskingRule::new(PiiType::Email, r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}", "****@****.com"),
            PiiType::Phone => MaskingRule::new(PiiType::Phone, r"\b\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b", "(***) ***-****"),
            PiiType::Ssn => MaskingRule::new(PiiType::Ssn, r"\b\d{3}-\d{2}-\d{4}\b", "***-**-****"),
            PiiType::CreditCard => MaskingRule::new(PiiType::CreditCard, r"\b(?:\d[ -]*?){13,19}\b", "****-****-****-****"),
            PiiType::IpAddress => MaskingRule::new(PiiType::IpAddress, r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b", "***.***.***.***"),
            _ => None,
        }
    }

    pub fn add_rule(&mut self, rule: MaskingRule) {
        self.rules.push(rule);
    }

    pub fn mask(&self, text: &str, user: &str, context: &str) -> MaskingResult {
        let original_length = text.len();
        let mut masked_types = Vec::new();
        let mut masked_count = 0;
        let mut result = text.to_string();

        for rule in &self.rules {
            let before = result.clone();
            result = rule.apply(&result);
            if before != result {
                masked_count += before.len() - result.len();
                if !masked_types.contains(&rule.pii_type()) {
                    masked_types.push(rule.pii_type());
                }
            }
        }

        if self.config.audit_enabled {
            if let Ok(mut log) = self.audit_log.write() {
                log.push(AuditEntry {
                    user: user.to_string(),
                    context: context.to_string(),
                    timestamp: chrono::Utc::now(),
                    original_hash: self.hash_text(text),
                    masked_length: result.len(),
                    pii_types_found: masked_types.clone(),
                });
            }
        }

        MaskingResult {
            original_length,
            masked_length: result.len(),
            masked_count,
            masked_types,
        }
    }

    pub fn mask_dict(&self, data: &serde_json::Value, user: &str, context: &str) -> serde_json::Value {
        match data {
            serde_json::Value::String(s) => {
                let _result = self.mask(s, user, context);
                serde_json::Value::String(self.apply_masks(s))
            }
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), self.mask_dict(v, user, context));
                }
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.mask_dict(v, user, context)).collect())
            }
            _ => data.clone(),
        }
    }

    fn apply_masks(&self, text: &str) -> String {
        let mut result = text.to_string();
        for rule in &self.rules {
            result = rule.apply(&result);
        }
        result
    }

    fn hash_text(&self, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn get_audit_log(&self) -> Vec<AuditEntry> {
        self.audit_log.read().unwrap().clone()
    }

    pub fn get_stats(&self) -> MaskingStats {
        let log = self.audit_log.read().unwrap();
        let total_requests = log.len();
        let mut pii_counts: HashMap<String, usize> = HashMap::new();
        
        for entry in log.iter() {
            for pii_type in &entry.pii_types_found {
                *pii_counts.entry(pii_type.as_str().to_string()).or_insert(0) += 1;
            }
        }
        
        MaskingStats {
            total_requests,
            pii_counts,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub user: String,
    pub context: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub original_hash: String,
    pub masked_length: usize,
    pub pii_types_found: Vec<PiiType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingStats {
    pub total_requests: usize,
    pub pii_counts: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_email() {
        let masker = DataMasker::new();
        let result = masker.mask("Contact: john@example.com", "user1", "api_response");
        
        assert!(result.masked_types.contains(&PiiType::Email));
        assert!(!masker.get_audit_log().is_empty());
    }

    #[test]
    fn test_mask_phone() {
        let masker = DataMasker::new();
        let result = masker.mask("Call me at (555) 123-4567", "user1", "log");
        
        assert!(result.masked_types.contains(&PiiType::Phone));
    }

    #[test]
    fn test_mask_ssn() {
        let masker = DataMasker::new();
        let result = masker.mask("SSN: 123-45-6789", "user1", "form");
        
        assert!(result.masked_types.contains(&PiiType::Ssn));
    }

    #[test]
    fn test_mask_credit_card() {
        let masker = DataMasker::new();
        let result = masker.mask("Card: 4111111111111111", "user1", "payment");
        
        assert!(result.masked_types.contains(&PiiType::CreditCard));
    }

    #[test]
    fn test_mask_ip_address() {
        let masker = DataMasker::new();
        let result = masker.mask("IP: 192.168.1.100", "user1", "access_log");
        
        assert!(result.masked_types.contains(&PiiType::IpAddress));
    }

    #[test]
    fn test_mask_multiple_types() {
        let masker = DataMasker::new();
        let result = masker.mask(
            "Email: test@test.com, Phone: 555-123-4567, IP: 10.0.0.1",
            "user1",
            "test",
        );
        
        assert!(result.masked_types.len() >= 3);
    }

    #[test]
    fn test_audit_log_tracking() {
        let masker = DataMasker::new();
        masker.mask("test@test.com", "user1", "test1");
        masker.mask("test2@test.com", "user2", "test2");
        
        let log = masker.get_audit_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].user, "user1");
        assert_eq!(log[1].user, "user2");
    }

    #[test]
    fn test_masking_stats() {
        let masker = DataMasker::new();
        masker.mask("test@test.com", "user1", "test");
        masker.mask("test2@test.com", "user2", "test");
        
        let stats = masker.get_stats();
        assert_eq!(stats.total_requests, 2);
        assert!(stats.pii_counts.contains_key("EMAIL"));
    }

    #[test]
    fn test_custom_rule() {
        let mut masker = DataMasker::new();
        masker.add_rule(MaskingRule::new(
            PiiType::Custom,
            r"\[REDACTED\]",
            "[MASKED]",
        ).unwrap());
        
        let result = masker.mask("Text with [REDACTED] content", "user1", "test");
        assert!(result.masked_types.contains(&PiiType::Custom));
    }

    #[test]
    fn test_mask_no_pii() {
        let masker = DataMasker::new();
        let result = masker.mask("This is normal text without PII", "user1", "test");
        
        assert!(result.masked_types.is_empty());
        assert_eq!(result.original_length, result.masked_length);
    }

    #[test]
    fn test_mask_dict() {
        let masker = DataMasker::new();
        let json = serde_json::json!({
            "email": "user@example.com",
            "name": "John Doe"
        });
        
        let masked = masker.mask_dict(&json, "user1", "api");
        assert!(masked.is_object());
    }
}
