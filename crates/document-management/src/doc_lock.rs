//! Document lock — concurrent edit control.
//!
//! Pessimistic locking to prevent conflicting edits.
//! Supports lock acquisition, release, timeout, and force-break.
//!
//! Reference: PostgreSQL Advisory Locks, Google Docs real-time collaboration locking model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Lock type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LockType {
    /// Exclusive lock — only one holder at a time.
    Exclusive,
    /// Shared lock — multiple readers, single writer.
    Shared,
}

/// Lock status for a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub doc_id: String,
    pub lock_type: LockType,
    pub holder: String,
    /// Unix timestamp millis when acquired.
    pub acquired_at_ms: u64,
    /// Unix timestamp millis when lock expires.
    pub expires_at_ms: u64,
    pub purpose: Option<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Document lock manager.
pub struct DocumentLockManager {
    locks: Mutex<HashMap<String, LockInfo>>,
    default_timeout: Duration,
    max_timeout: Duration,
}

impl DocumentLockManager {
    pub fn new(default_timeout: Duration, max_timeout: Duration) -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
            default_timeout,
            max_timeout,
        }
    }

    /// Acquire a lock on a document.
    pub fn acquire(
        &self,
        doc_id: &str,
        holder: &str,
        lock_type: LockType,
        purpose: Option<String>,
    ) -> Result<LockInfo, String> {
        self.acquire_with_timeout(doc_id, holder, lock_type, self.default_timeout, purpose)
    }

    /// Acquire a lock with a specific timeout.
    pub fn acquire_with_timeout(
        &self,
        doc_id: &str,
        holder: &str,
        lock_type: LockType,
        timeout: Duration,
        purpose: Option<String>,
    ) -> Result<LockInfo, String> {
        let timeout = timeout.min(self.max_timeout);
        let now = now_ms();
        let timeout_ms = timeout.as_millis() as u64;
        let mut locks = self.locks.lock().map_err(|e| e.to_string())?;

        locks.retain(|_, lock| lock.expires_at_ms > now);

        if let Some(existing) = locks.get(doc_id) {
            if existing.holder == holder {
                let info = LockInfo {
                    doc_id: doc_id.to_string(),
                    lock_type,
                    holder: holder.to_string(),
                    acquired_at_ms: now,
                    expires_at_ms: now + timeout_ms,
                    purpose,
                };
                locks.insert(doc_id.to_string(), info.clone());
                return Ok(info);
            }

            if existing.expires_at_ms <= now {
                // Expired, fall through
            } else if existing.lock_type == LockType::Exclusive || lock_type == LockType::Exclusive
            {
                return Err(format!(
                    "Document '{}' is locked by '{}' (expires in {}ms)",
                    doc_id,
                    existing.holder,
                    existing.expires_at_ms - now
                ));
            }
        }

        let info = LockInfo {
            doc_id: doc_id.to_string(),
            lock_type,
            holder: holder.to_string(),
            acquired_at_ms: now,
            expires_at_ms: now + timeout_ms,
            purpose,
        };
        locks.insert(doc_id.to_string(), info.clone());
        Ok(info)
    }

    /// Release a lock.
    pub fn release(&self, doc_id: &str, holder: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().map_err(|e| e.to_string())?;
        if let Some(existing) = locks.get(doc_id) {
            if existing.holder != holder {
                return Err(format!(
                    "Cannot release lock held by '{}', you are '{}'",
                    existing.holder, holder
                ));
            }
        }
        locks.remove(doc_id);
        Ok(())
    }

    /// Force-break a lock (admin action).
    pub fn force_break(&self, doc_id: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().map_err(|e| e.to_string())?;
        locks.remove(doc_id);
        Ok(())
    }

    /// Extend a lock's timeout.
    pub fn extend(&self, doc_id: &str, holder: &str, additional: Duration) -> Result<(), String> {
        let mut locks = self.locks.lock().map_err(|e| e.to_string())?;
        if let Some(lock) = locks.get_mut(doc_id) {
            if lock.holder != holder {
                return Err("Not the lock holder".to_string());
            }
            let add_ms = additional.as_millis() as u64;
            let max_ms = self.max_timeout.as_millis() as u64;
            lock.expires_at_ms = (lock.expires_at_ms + add_ms).min(now_ms() + max_ms);
            Ok(())
        } else {
            Err(format!("No lock on document '{}'", doc_id))
        }
    }

    /// Check if a document is locked.
    pub fn is_locked(&self, doc_id: &str) -> bool {
        let locks = self.locks.lock().unwrap_or_else(|e| e.into_inner());
        locks
            .get(doc_id)
            .map(|l| l.expires_at_ms > now_ms())
            .unwrap_or(false)
    }

    /// Get lock info for a document.
    pub fn get_lock_info(&self, doc_id: &str) -> Option<LockInfo> {
        let locks = self.locks.lock().ok()?;
        locks.get(doc_id).and_then(|lock| {
            if lock.expires_at_ms > now_ms() {
                Some(lock.clone())
            } else {
                None
            }
        })
    }

    /// List all active locks.
    pub fn list_locks(&self) -> Vec<LockInfo> {
        let locks = self.locks.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_ms();
        locks
            .values()
            .filter(|l| l.expires_at_ms > now)
            .cloned()
            .collect()
    }

    /// Clean up expired locks.
    pub fn cleanup_expired(&self) -> usize {
        let mut locks = self.locks.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_ms();
        let before = locks.len();
        locks.retain(|_, lock| lock.expires_at_ms > now);
        before - locks.len()
    }
}

impl Default for DocumentLockManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(300), Duration::from_secs(3600))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_and_release() {
        let mgr = DocumentLockManager::default();
        let lock = mgr
            .acquire("doc1", "user1", LockType::Exclusive, None)
            .unwrap();
        assert_eq!(lock.holder, "user1");
        assert!(mgr.is_locked("doc1"));
        mgr.release("doc1", "user1").unwrap();
        assert!(!mgr.is_locked("doc1"));
    }

    #[test]
    fn test_exclusive_lock_conflict() {
        let mgr = DocumentLockManager::default();
        mgr.acquire("doc1", "user1", LockType::Exclusive, None)
            .unwrap();
        assert!(mgr
            .acquire("doc1", "user2", LockType::Exclusive, None)
            .is_err());
    }

    #[test]
    fn test_shared_locks_allowed() {
        let mgr = DocumentLockManager::default();
        mgr.acquire("doc1", "user1", LockType::Shared, None)
            .unwrap();
        assert!(mgr.acquire("doc1", "user2", LockType::Shared, None).is_ok());
    }

    #[test]
    fn test_force_break() {
        let mgr = DocumentLockManager::default();
        mgr.acquire("doc1", "user1", LockType::Exclusive, None)
            .unwrap();
        mgr.force_break("doc1").unwrap();
        assert!(!mgr.is_locked("doc1"));
    }

    #[test]
    fn test_extend() {
        let mgr = DocumentLockManager::default();
        mgr.acquire("doc1", "user1", LockType::Exclusive, None)
            .unwrap();
        mgr.extend("doc1", "user1", Duration::from_secs(60))
            .unwrap();
        let info = mgr.get_lock_info("doc1").unwrap();
        assert!(info.expires_at_ms > now_ms() + 50_000);
    }

    #[test]
    fn test_reacquire_by_same_holder() {
        let mgr = DocumentLockManager::default();
        mgr.acquire("doc1", "user1", LockType::Exclusive, None)
            .unwrap();
        assert!(mgr
            .acquire("doc1", "user1", LockType::Exclusive, None)
            .is_ok());
    }
}
