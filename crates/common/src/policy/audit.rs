//! PolicyAuditLog - Immutable linked list for audit logging
//!
//! Uses a persistent/immutable linked list data structure where each entry
//! points to the previous entry, allowing efficient append and full history
//! traversal without mutation.

use serde::{Deserialize, Serialize};

use super::engine::PolicyEvaluationResult;

/// A single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Resource that was evaluated
    pub resource: String,
    /// Action that was performed
    pub action: String,
    /// Evaluation result
    pub result: PolicyEvaluationResult,
    /// Timestamp (ISO 8601 format)
    pub timestamp: String,
}

/// PolicyAuditLog - An immutable linked list implementation for audit logging
///
/// Each append creates a new head node pointing to the previous chain.
/// The original log remains unchanged, providing immutable history.
#[derive(Debug, Clone)]
pub struct PolicyAuditLog {
    head: Option<AuditEntry>,
    /// Pointer to the previous log (for immutable append)
    prev: Option<Box<PolicyAuditLog>>,
    len: usize,
}

impl PolicyAuditLog {
    /// Creates a new empty audit log
    pub fn new() -> Self {
        Self {
            head: None,
            prev: None,
            len: 0,
        }
    }

    /// Returns the number of entries in the audit log
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the audit log is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends a new entry to the audit log, returning a new log instance
    ///
    /// Note: This creates a new immutable log with the entry prepended.
    /// The original log remains unchanged.
    pub fn append(
        &self,
        resource: &str,
        action: &str,
        result: &PolicyEvaluationResult,
    ) -> Self {
        let entry = AuditEntry {
            resource: resource.to_string(),
            action: action.to_string(),
            result: result.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        Self {
            head: Some(entry),
            prev: Some(Box::new(Self {
                head: self.head.clone(),
                prev: self.prev.clone(),
                len: self.len,
            })),
            len: self.len + 1,
        }
    }

    /// Returns an iterator over all audit entries (newest first)
    pub fn iter(&self) -> AuditLogIterator {
        AuditLogIterator {
            current_entry: self.head.clone(),
            current_prev: self.prev.clone(),
        }
    }

    /// Returns all entries as a Vec (newest first)
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.iter().collect()
    }
}

impl Default for PolicyAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over audit log entries (newest first)
#[derive(Debug, Clone)]
pub struct AuditLogIterator {
    current_entry: Option<AuditEntry>,
    current_prev: Option<Box<PolicyAuditLog>>,
}

impl Iterator for AuditLogIterator {
    type Item = AuditEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_entry.take() {
            Some(entry) => {
                if let Some(ref prev_log) = self.current_prev {
                    self.current_entry = prev_log.head.clone();
                    self.current_prev = prev_log.prev.clone();
                } else {
                    self.current_entry = None;
                }
                Some(entry)
            }
            None => None,
        }
    }
}

impl IntoIterator for PolicyAuditLog {
    type Item = AuditEntry;
    type IntoIter = AuditLogIterator;

    fn into_iter(self) -> Self::IntoIter {
        AuditLogIterator {
            current_entry: self.head,
            current_prev: self.prev,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Effect;

    fn create_result(allowed: bool) -> PolicyEvaluationResult {
        PolicyEvaluationResult {
            allowed,
            matched_rule_id: Some("test-rule".to_string()),
            reason: "Test".to_string(),
        }
    }

    #[test]
    fn test_new_log_is_empty() {
        let log = PolicyAuditLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_append_increases_length() {
        let log = PolicyAuditLog::new();
        let result = create_result(true);
        let new_log = log.append("resource1", "action1", &result);

        assert_eq!(new_log.len(), 1);
        assert!(!new_log.is_empty());
        // Original log unchanged
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_append_chain() {
        let log = PolicyAuditLog::new();
        let result1 = create_result(true);
        let result2 = create_result(false);

        let log1 = log.append("resource1", "action1", &result1);
        let log2 = log1.append("resource2", "action2", &result2);

        assert_eq!(log2.len(), 2);
        assert_eq!(log.len(), 0);
        assert_eq!(log1.len(), 1);
    }

    #[test]
    fn test_iter_order_newest_first() {
        let log = PolicyAuditLog::new();
        let result = create_result(true);

        let log = log.append("first", "action", &result);
        let log = log.append("second", "action", &result);
        let log = log.append("third", "action", &result);

        let entries: Vec<_> = log.iter().collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].resource, "third");
        assert_eq!(entries[1].resource, "second");
        assert_eq!(entries[2].resource, "first");
    }

    #[test]
    fn test_entries() {
        let log = PolicyAuditLog::new();
        let result = create_result(true);

        let log = log.append("res1", "act1", &result);
        let log = log.append("res2", "act2", &result);

        let entries = log.entries();
        assert_eq!(entries[0].resource, "res2");
        assert_eq!(entries[1].resource, "res1");
    }

    #[test]
    fn test_clone_preserves_history() {
        let log1 = PolicyAuditLog::new();
        let result = create_result(true);
        let log2 = log1.append("res1", "act1", &result);
        let log3 = log2.append("res2", "act2", &result);

        let log2_clone = log2.clone();
        assert_eq!(log2_clone.len(), 1);
        assert_eq!(log2_clone.entries()[0].resource, "res1");

        // log3 should have both entries
        let entries = log3.entries();
        assert_eq!(entries.len(), 2);
    }
}
